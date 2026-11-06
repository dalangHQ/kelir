#![allow(clippy::unwrap_used)]
#![allow(dead_code)]

// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use kelir_interface::driver::{ExternalSuspendKind, SuspendReason};
use kelir_interface::plugin::HandlerCtx as IfaceHandlerCtx;
use tokio::sync::Mutex;

use kelir::{
    HCons, HNil, KelirBuilder, NoParams, PidLineage, PostExecAction, SnapshotDescriptor, TrapCtx,
    TrapFilter, Unsealed, Verdict, VmConfig,
    driver::{DriverPolicy, WxPolicy},
    handler,
    planes::{
        storage::{
            DynamicFile, IdmapConfig, MaskRule, ReadMaskEngine, SnoopHandler, SnoopRule,
            StorageConfig, SyntheticFile, WorkspaceEncapsulation,
        },
        terminal::{PromptRule, PtyConfig, TokenShield},
        wire::{L4Proxy, Mtls, SinkholeConfig, TapFilter, VaultEntry, WireConfig},
    },
    plugin::{AnyPlugin, FilterAction, FsInterposer, KelirPlugin, PluginError, TrapInterposer},
    plugins::{
        chaos::{
            ChaosConfig, ChaosPlugin, FaultDirection, FaultInjectionService, NetworkFaultRule,
            StorageFaultRule,
        },
        ledger::LedgerPlugin,
    },
    protocol::{PatchPayload, TrapEvent},
    telemetry::ExecutionReceipt,
};

#[derive(Debug, Clone)]
pub struct RepoPolicyOptions {
    pub max_diff_bytes: usize,
    pub protected_branches: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct RepoPolicyState {
    violations: usize,
    current_branch: String,
}

#[derive(Clone)]
pub struct RepoPolicyHandle {
    state: Arc<Mutex<RepoPolicyState>>,
    protected: Arc<Vec<String>>,
}

impl RepoPolicyHandle {
    pub async fn violations(&self) -> usize {
        self.state.lock().await.violations
    }
    pub async fn branch(&self) -> String {
        self.state.lock().await.current_branch.clone()
    }
    pub async fn record_violation(&self) {
        self.state.lock().await.violations += 1;
    }
    pub async fn set_branch(&self, b: impl Into<String>) {
        self.state.lock().await.current_branch = b.into();
    }
    pub async fn is_protected(&self) -> bool {
        let s = self.state.lock().await;
        self.protected.iter().any(|p| {
            p == &s.current_branch
                || p.strip_suffix('*')
                    .is_some_and(|prefix| s.current_branch.starts_with(prefix))
        })
    }
}

#[derive(Clone)]
pub struct RepoPolicyPlugin {
    opts: RepoPolicyOptions,
    state: Arc<Mutex<RepoPolicyState>>,
}

impl RepoPolicyPlugin {
    pub fn new(opts: RepoPolicyOptions) -> Self {
        Self {
            opts,
            state: Arc::new(Mutex::new(RepoPolicyState::default())),
        }
    }
}

impl Default for RepoPolicyHandle {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(RepoPolicyState::default())),
            protected: Arc::new(Vec::new()),
        }
    }
}

impl RepoPolicyPlugin {
    pub fn handle(&self) -> RepoPolicyHandle {
        RepoPolicyHandle {
            state: Arc::clone(&self.state),
            protected: Arc::new(self.opts.protected_branches.clone()),
        }
    }
}

impl KelirPlugin for RepoPolicyPlugin {
    type Context = RepoPolicyHandle;
    fn context(&self) -> Self::Context {
        self.handle()
    }
    fn name(&self) -> &'static str {
        "repo-policy"
    }

    fn fork_state(&self, _child_session_id: &str) -> Box<dyn AnyPlugin> {
        Box::new(Self {
            opts: self.opts.clone(),
            state: Arc::new(Mutex::new(self.state.blocking_lock().clone())),
        })
    }
}

#[async_trait]
impl TrapInterposer for RepoPolicyPlugin {
    async fn on_trap(
        &self,
        event: &TrapEvent,
        _ctx: &IfaceHandlerCtx,
    ) -> Result<Verdict, PluginError> {
        let is_git = event.binary_path.ends_with("/git") || event.binary_path == "git";
        let is_push = event.argv.get(1).map(String::as_str) == Some("push");
        if !(is_git && is_push) {
            return Ok(Verdict::Pass);
        }

        let h = self.handle();
        if h.is_protected().await {
            h.record_violation().await;
            return Ok(Verdict::Drop {
                errno: Some("EPERM".into()),
                reason: format!("EPOLICY: '{}' is protected.", h.branch().await),
            });
        }

        let new_branch = format!("agent/{:08x}", std::process::id());
        let mut argv = event.argv.clone();
        argv.push(format!("HEAD:refs/heads/{new_branch}"));

        let mutation = serde_json::json!({
            "new_binary": "/bin/git",
            "new_argv": argv,
            "set_env": { "KELIR_REWRITTEN": "1" },
            "unset_env": [],
        });
        Ok(Verdict::Patch(PatchPayload::ExecveMutation(
            serde_json::to_vec(&mutation).map_err(|e| PluginError::Halt(e.to_string()))?,
        )))
    }
}

impl FsInterposer for RepoPolicyPlugin {
    // see: cans/02-plugin-model.md#Segregated-ring-traits
    fn on_fs_write(
        &self,
        path: &Path,
        data: &mut [u8],
        _pid: u32,
        _ctx: &IfaceHandlerCtx,
    ) -> Result<FilterAction, PluginError> {
        if path.extension().is_some_and(|e| e == "diff") && data.len() > self.opts.max_diff_bytes {
            self.state.blocking_lock().violations += 1;
            return Ok(FilterAction::Drop {
                errno: Some("ENOSPC".into()),
                reason: "diff exceeds policy ceiling".into(),
            });
        }
        Ok(FilterAction::Forward)
    }
}

impl RepoPolicyPlugin {
    async fn disclosure_annotation(&self) -> Vec<u8> {
        format!(
            "[repo-policy: {} open violations]\n",
            self.handle().violations().await
        )
        .into_bytes()
    }
}

async fn build_runtime(
    oracle: Arc<UserOracle>,
    pg_cert: PathBuf,
    pg_key: PathBuf,
) -> Result<
    KelirBuilder<
        Unsealed,
        HCons<RepoPolicyHandle, HCons<(), HCons<Arc<dyn FaultInjectionService>, HNil>>>,
        NoParams,
    >,
    Box<dyn std::error::Error + Send + Sync>,
> {
    let policy_plugin = RepoPolicyPlugin::new(RepoPolicyOptions {
        max_diff_bytes: 64 * 1024,
        protected_branches: vec!["main".into(), "master".into(), "release/*".into()],
    });

    // see: cans/02-plugin-model.md#Shape-of-the-surface
    let snoop_oracle = Arc::clone(&oracle);
    let snoop: SnoopHandler = Arc::new(move |path: PathBuf, raw: Vec<u8>, _c: TrapCtx<_, _>| {
        let evt: serde_json::Value = serde_json::from_slice(&raw)
            .map_err(|e| PluginError::Halt(format!("session is not JSON: {e}")))?;
        if evt
            .get("falsifiedResult")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return Err(PluginError::Halt(
                "EPERM: harness hallucination detected".into(),
            ));
        }
        // TODO (L3): attribute the producing binary via c.ctx::<LedgerCtx>() once plugin-ledger contributes its context handle per cans/plugins/ledger.md.
        let blake3 = String::new();

        snoop_oracle
            .ingest_harness_event(&path, &evt, &blake3)
            .map_err(|e| PluginError::Halt(e.to_string()))?;
        Ok(())
    });

    let builder = KelirBuilder::new(VmConfig {
        vcpus: 4,
        memory_mb: 4096,
        kernel_path: format!("./fixtures/vmlinux-6.6-{}-kelir", std::env::consts::ARCH).into(),
        rootfs_erofs_path: "./fixtures/rootfs-swebench.erofs".into(),
        tsc_locked: true,
        enable_watchdog: true,
        enable_gpu_passthrough: false,
        driver_policy: DriverPolicy {
            require_hardware_vmm: false,
            allow_emulated_fallback: true,
            verify_hw_lockfile: false,
            wx_enforcement: WxPolicy::PermissiveAudit,
            ffi_worker_threads: 2,
            ffi_max_ops_per_sec: 1000,
        },
        page_size_kb: 4,
        deterministic_clock: false,
        network_off: false,
        pinned_clocksource: None,
        ns_per_instruction: None,
        preemption_grace_ms: 0,
        entropy_seed: None,
    })
    .terminal(PtyConfig {
        cols: 120,
        rows: 40,
        drop_osc52: true,
        strip_title: true,
        token_shield: Some(TokenShield {
            max_bytes_total: 128 * 1024,
            max_lines_total: 500,
            clamp_strategy: "head-and-tail".into(),
            notice: "⚠️ [SHIELD]".into(),
        }),
        prompts: vec![PromptRule {
            pattern: r"(?i)Overwrite production schema\? \[y/N\]".into(),
            timeout: Duration::from_secs(3),
            verify_foreground: true,
            action: Arc::new(|_, stream| {
                Box::pin(async move {
                    stream.write(b"y\n").await?;
                    Ok(())
                })
            }),
        }],

        stream_engine: "StatelessRegex".into(),
    })
    .storage(StorageConfig {
        idmap: IdmapConfig {
            guest_uid: 0,

            host_uid: 0,
        },
        shared_caches: vec![
            "~/.npm".into(),
            "~/.cache/pip".into(),
            "~/.cargo/registry".into(),
        ],
        lock_broker: true,
        synthetic_files: vec![
            SyntheticFile::static_(
                "/workspace/.env.local",
                "DATABASE_URL=postgres://app:vault@127.0.0.1:5432/prod\n",
            ),
            SyntheticFile::dynamic("/workspace/identity.jwt", |_| {
                Ok(DynamicFile {
                    content: format!("Bearer {:032x}", std::process::id()),
                    mode: 0o400,
                })
            }),
        ],
        mask_read: vec![MaskRule {
            glob: "**/secrets/**".into(),
            patterns: vec![
                (
                    r"sk_live_[0-9a-zA-Z]{24}".into(),
                    "[STRIPE_VAULT_KEY]".into(),
                ),
                (r"AKIA[0-9A-Z]{16}".into(), "[AWS_IAM_REDACTED]".into()),
            ],
        }],
        snoops: vec![SnoopRule {
            glob: "~/.claude/sessions/*.json".into(),
            sync: true,
            on_write: snoop,
        }],
        encapsulation: WorkspaceEncapsulation::Passthrough,
        scan_engine: ReadMaskEngine::FastRegex,
        carryover_bytes: 1024,
    })
    .wire(WireConfig {
        sinkhole: SinkholeConfig {
            allowlist: vec![
                "api.stripe.com".into(),
                "github.com".into(),
                "*.internal.net".into(),
            ],
            drop_all: true,
        },
        ssh_agent_bridge: true,

        vaults: vec![VaultEntry {
            name: "STRIPE_SECRET".into(),
            placeholder: "vault://stripe_key".into(),
        }],
        l4_proxies: vec![L4Proxy {
            listen_port: 5432,
            destination: "pg.internal.net:5432".into(),
            inject_mtls: Some(Mtls {
                cert_path: pg_cert,
                key_path: pg_key,
            }),
        }],

        tap_filter: TapFilter::AllowUdpProxied,
    })
    .plugin(ChaosPlugin::new(ChaosConfig {
        network: vec![
            NetworkFaultRule {
                target: "api.stripe.com".into(),
                direction: FaultDirection::Outbound,
                base_ms: 150,
                jitter_ms: 30,
                drop_rate: 0.02,
                inject_rst: false,
                bandwidth_kbps: None,
            },
            NetworkFaultRule {
                target: "api.stripe.com".into(),
                direction: FaultDirection::Inbound,
                base_ms: 150,
                jitter_ms: 30,
                drop_rate: 0.02,
                inject_rst: false,
                bandwidth_kbps: None,
            },
            NetworkFaultRule {
                target: "api.stripe.com".into(),
                direction: FaultDirection::Both,
                base_ms: 150,
                jitter_ms: 30,
                drop_rate: 0.02,
                inject_rst: false,
                bandwidth_kbps: None,
            },
        ],
        fs: vec![StorageFaultRule {
            target_path: "/workspace/build/cache/*".into(),
            simulate_error: "ENOSPC".into(),
            error_probability: 0.05,
        }],
    }))
    // TODO (L3): mount the ledger config per cans/artifacts/examples/autonomous-swe-agent.rs once plugin-ledger owns LedgerConfig per cans/plugins/ledger.md.
    .plugin(LedgerPlugin::new(true));

    let builder = builder.plugin(policy_plugin);

    let builder = builder

        .trap("*", handler({
            let oracle = Arc::clone(&oracle);
            move |c: TrapCtx<_, _>| {
                let oracle = Arc::clone(&oracle);
                async move {

                    let _pre = c.vm
                        .checkpoint(format!("pre:{}", c.trap.pid))
                        .await?;

                    let intent: IntentAnalysis = oracle.analyze_intent(&c.trap).await?;
                    if !intent.is_hypothesis_valid {
                        return Ok(Verdict::Drop {
                            errno: Some("EPERM".into()),
                            reason: format!(
                                "Verification Reject: {}",
                                intent.falsification_reason.unwrap_or_default()
                            ),
                        });
                    }
                    Ok(Verdict::Pass)
                }
            }
        }))


        .trap(TrapFilter::binary("npm").with_arg("test").with_arg("--").with_arg("--runInBand"), handler(|c: TrapCtx<_, _>| async move {
            c.fs.warm_cache(&["node_modules", ".next/cache"]).await?;


            Ok(Verdict::Patch(PatchPayload::ExecveMutation(serde_json::to_vec(&serde_json::json!({
                "new_binary": c.trap.binary_path.clone(),
                "new_argv": c.trap.argv.clone(),
                "set_env": { "OTEL_TRACE_ID": format!("{:032x}", std::process::id()), "CI": "1" },
                "unset_env": [],
            }))?)))
        }))


        .trap(TrapFilter::binary("git").with_arg("push"), handler(|c: TrapCtx<_, _>| async move {
            if c.trap.argv.iter().any(|a| a == "--force") {

                let argv: Vec<String> = c.trap.argv.iter()
                    .filter(|a| *a != "--force").cloned().collect();
                return Ok(Verdict::Patch(PatchPayload::ExecveMutation(serde_json::to_vec(&serde_json::json!({
                    "new_binary": "/bin/git",
                    "new_argv": argv,
                    "set_env": { "KELIR_DISARMED": "1" },
                    "unset_env": [],
                }))?)));
            }
            Ok(Verdict::Pass)
        }))


        .trap("*", handler({
            let oracle = Arc::clone(&oracle);
            move |c: TrapCtx<_, _>| {
                let oracle = Arc::clone(&oracle);
                async move {
                    let pre: SnapshotDescriptor = c.step.last().ok_or_else(|| {
                        PluginError::Halt("no pre-exec checkpoint".into())
                    })?;
                    let receipt: ExecutionReceipt = c.receipt().await?;


                    let trap_hex: String = c.trap.binary_blake3.iter().fold(
                        String::with_capacity(64),
                        |mut hex, b| {
                            hex.push_str(&format!("{:02x}", b));
                            hex
                        },
                    );
                    if receipt.binary_blake3 != trap_hex {
                        c.vm.rollback(&pre).await?;
                        c.terminal.notice(
                            "⚡ [KELIR]: post-exec binary hash mismatch — REVERTED\n",
                        );
                        return Ok(PostExecAction::RollbackAndHalt);
                    }

                    if c.trap.binary_path.ends_with("/npm")
                        && receipt.egress_bytes > 0
                        && c.trap.cwd.contains("/checkout-service")
                    {
                        c.vm.rollback(&pre).await?;
                        c.terminal.notice(&format!(
                            "⚡ [KELIR]: hermetic test emitted {}B egress — REVERTED\n",
                            receipt.egress_bytes
                        ));
                        return Ok(PostExecAction::RollbackAndHalt);
                    }

                    if !oracle.verify_physical_receipt(&receipt).await? {
                        c.vm.rollback(&pre).await?;
                        c.terminal.notice(
                            "⚡ [KELIR]: post-condition violated — REVERTED\n",
                        );
                        return Ok(PostExecAction::RollbackAndHalt);
                    }
                    Ok(PostExecAction::Proceed)
                }
            }
        }))


        .trap(TrapFilter::binary("git").with_arg("commit").with_arg("-m"), handler({
            move |c: TrapCtx<_, _>| async move {


                let policy = c.ctx::<RepoPolicyHandle, _>();
                let n = policy.violations().await;
                if n > 0 {
                    return Ok(Verdict::Drop {
                        errno: Some("EPERM".into()),
                        reason: format!("EPOLICY: {n} prior violation(s)."),
                    });
                }
                Ok(Verdict::Pass)
            }
        }))


        .trap(TrapFilter::binary("git").with_arg("commit"), handler(|c: TrapCtx<_, _>| async move {
            let receipt = c.receipt().await?;

            if receipt.dirty_bytes == 0 {

                return Err(Box::new(PluginError::Halt("zero-byte delta commit".into()))
                    as Box<dyn std::error::Error + Send + Sync>);
            }


            match receipt.extensions.iter().find(|e| e.namespace == "fs:vfs") {
                Some(ext) => eprintln!(
                    "[kelir] commit fs:vfs extension present, payload_len={}",
                    ext.payload.len(),
                ),
                None => eprintln!("[kelir] commit fs:vfs extension absent"),
            }
            Ok(PostExecAction::Proceed)
        }));

    let builder = builder
        .on_vm_init(|driver| async move {
            driver
                .set_watchdog(Duration::from_secs(5 * 60), kelir::WatchdogAction::Reboot)
                .await
        })
        .on_vm_ready(|session| {
            let result = session.terminal.resize(120, 40);
            async move { result }
        })
        .on_vm_shutdown(|_session| async move { Ok::<(), PluginError>(()) })
        .on_trap_deny(|_event, _session| async move { Ok::<(), PluginError>(()) });

    Ok(builder)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let oracle: Arc<UserOracle> = Arc::new(UserOracle::new());

    let builder = build_runtime(
        Arc::clone(&oracle),
        "/etc/ssl/client.crt".into(),
        "/etc/ssl/client.key".into(),
    )
    .await?;

    let mut session = builder.boot().await?;
    session.terminal.resize(120, 40)?;
    eprintln!("[kelir] host pressure={}", session.host.pressure_level());

    let port = session.wire.forward_port(3000, Some(8080))?;
    eprintln!("[kelir] wire forward_port -> {port}");

    session.checkpoint.create_step(
        PidLineage::new(1),
        SnapshotDescriptor {
            id: "clean_base".into(),
            dirty_bytes: 0,
            created_at_ns: 0,
            parent_digest: None,
            guest_clock: None,
        },
    )?;
    let base = session.checkpoint.commit_step(PidLineage::new(1))?;
    eprintln!("[kelir] checkpoint base node={base}");

    let phase: Result<(), Box<dyn std::error::Error + Send + Sync>> = async {
        let mut joins = tokio::task::JoinSet::new();
        for id in ['A', 'B', 'C', 'D'] {
            let mut child = session.fork()?;
            joins.spawn(async move {
                child.checkpoint.create_step(
                    PidLineage::new(1),
                    SnapshotDescriptor {
                        id: format!("hypothesis-{id}"),
                        dirty_bytes: 0,
                        created_at_ns: 0,
                        parent_digest: None,
                        guest_clock: None,
                    },
                )?;
                child.checkpoint.commit_step(PidLineage::new(1))?;
                // TODO (L3): skew the child clock, spawn npm run verify-hypothesis-{id}, and read back its causal delta.
                child.stop()?;
                Ok::<_, Box<dyn std::error::Error + Send + Sync>>(id)
            });
        }

        let mut winner: Option<char> = None;
        while let Some(joined) = joins.join_next().await {
            let id = joined??;
            if winner.is_none() {
                winner = Some(id);
            }
        }
        let Some(id) = winner else {
            session.rollback()?;
            return Ok(());
        };

        session.suspend(SuspendReason::External(
            ExternalSuspendKind::HumanAuthorizationHold,
        ))?;
        eprintln!("[kelir] suspended: host RSS = 0 MB; awaiting approval");
        oracle.await_human_approval().await?;
        session.resume()?;

        session.storage.reconcile()?;
        eprintln!("[kelir] broker commit done for winner {id}");
        // TODO (L3): spawn npm test -- --runInBand, git commit the winning fix, and finalize the ledger receipt.

        Ok(())
    }
    .await;

    if let Err(e) = session.suspend(SuspendReason::External(
        ExternalSuspendKind::HumanAuthorizationHold,
    )) {
        eprintln!("[kelir] final suspend failed: {e}");
    }
    session.stop()?;
    phase
}

pub struct IntentAnalysis {
    pub is_hypothesis_valid: bool,
    pub falsification_reason: Option<String>,
    pub allow_high_entropy: bool,
}

struct UserOracle;
impl UserOracle {
    fn new() -> Self {
        Self
    }
}

impl UserOracle {
    async fn analyze_intent(
        &self,
        _trap: &TrapEvent,
    ) -> Result<IntentAnalysis, Box<dyn std::error::Error + Send + Sync>> {
        Ok(IntentAnalysis {
            is_hypothesis_valid: true,
            falsification_reason: None,
            allow_high_entropy: false,
        })
    }
    async fn verify_physical_receipt(
        &self,
        _r: &ExecutionReceipt,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(true)
    }
    // see: cans/02-plugin-model.md#Shape-of-the-surface
    fn ingest_harness_event(
        &self,
        _p: &Path,
        _e: &serde_json::Value,
        _b: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    async fn await_human_approval(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(())
    }
}
