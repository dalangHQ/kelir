#![allow(dead_code)]

// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
// see: cans/plugins/ledger.md#Plugin-ledger
// see: cans/plugins/chaos.md#Plugin-chaos

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use kelir_interface::credential_vault::{CredentialDescriptor, CredentialKind};
use kelir_interface::driver_interface::{ExternalSuspendKind, SuspendReason};
use kelir_interface::plugin_model::HandlerCtx as IfaceHandlerCtx;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use kelir::{
    driver_interface::{DriverPolicy, WxPolicy},
    handler,
    planes::{
        storage_plane::{
            DynamicFile, IdmapConfig, MaskRule, ReadMaskEngine, SnoopHandler, SnoopRule,
            StorageConfig, SyntheticFile, WorkspaceEncapsulation,
        },
        terminal_plane::{PromptRule, PtyConfig, TokenShield},
        wire_plane::{L4Proxy, Mtls, SinkholeConfig, TapFilter, WireConfig},
    },
    plugin_model::{
        AnyPlugin, FilterAction, FsInterposer, KelirPlugin, PluginError, TrapInterposer,
    },
    plugins::{
        plugin_chaos::{
            ChaosConfig, ChaosPlugin, FaultDirection, FaultInjectionService, NetworkFaultRule,
            StorageFaultRule,
        },
        plugin_ledger::{LedgerConfig, LedgerCtx, LedgerPlugin, OtelConfig, ReceiptConfig},
    },
    protocol_wire::{PatchPayload, TrapEvent},
    telemetry_attest::ExecutionReceipt,
    FsReconcileRequest, HCons, HNil, HandlerCtx, KelirBuilder, NoParams, PostExecAction,
    SnapshotDescriptor, TrapFilter, Unsealed, Verdict, VmConfig,
};

// see: cans/03-sdk-architecture.md#.params(model:-type[P])-pins-Params;-handler-c.params-is-typed
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum SpawnAction {
    FixStripeWebhook {
        repo: String,
        issue_id: String,
        env: Env,
    },
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Env {
    Staging,
    Production,
}

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
        HCons<RepoPolicyHandle, HCons<LedgerCtx, HCons<Arc<dyn FaultInjectionService>, HNil>>>,
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
    let snoop: SnoopHandler = Arc::new(move |path: PathBuf, raw: Vec<u8>, _c: HandlerCtx<_, _>| {
        let evt: serde_json::Value = serde_json::from_slice(&raw)
            .map_err(|e| PluginError::Halt(format!("session is not JSON: {e}")))?;
        if evt
            .get("falsifiedResult")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return Err(PluginError::reject(
                "EPERM",
                "harness hallucination detected",
            ));
        }
        // TODO (L3): attribute via c.ctx::<LedgerCtx>().blake3_of_producing_process(&path).await once SnoopHandler carries the artifact async form.
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
            host_uid: unsafe { libc::getuid() },
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

        vaults: vec![CredentialDescriptor {
            name: "STRIPE_SECRET".into(),
            kind: CredentialKind::BearerToken,
            audience: "api.stripe.com".into(),
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
    // see: cans/plugins/ledger.md#Plugin-ledger
    .plugin(LedgerPlugin::new(true).with_config(LedgerConfig {
        store: ".kelir/audit.merkle.db".into(),
        merkle_chain: true,
        otel: Some(OtelConfig {
            endpoint: "http://localhost:4318/v1/traces".into(),
            service_name: "kelir-swe-agent".into(),
            propagate_w3c_traceparent: true,
            export_diagnostic_pmu_gauges: false,
        }),
        receipts: ReceiptConfig {
            include_dirty_bytes: true,
            include_binary_blake3: true,
            include_syscall_count: true,
            include_wall_clock_ns: true,
            include_phase_timings: false,
        },
    }));

    let builder = builder.plugin(policy_plugin);

    let builder = builder

        .trap("*", handler({
            let oracle = Arc::clone(&oracle);
            move |c: HandlerCtx<_, _>| {
                let oracle = Arc::clone(&oracle);
                async move {

                    let _pre = c.vm
                        .checkpoint(format!("pre:{}", c.trap.pid))
                        .await?;

                    let intent: IntentAnalysis = oracle.analyze_intent(&c.trap).await?;
                    if !intent.is_hypothesis_valid {
                        // see: cans/plugins/ledger.md#Ledger-handler-API
                        c.ctx::<LedgerCtx, _>().record_denial(c.trap.ppid);
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


        .trap(TrapFilter::binary("npm").with_arg("test").with_arg("--").with_arg("--runInBand"), handler(|c: HandlerCtx<_, _>| async move {
            c.fs.warm_cache(&["node_modules", ".next/cache"]).await?;


            Ok(Verdict::Patch(PatchPayload::ExecveMutation(serde_json::to_vec(&serde_json::json!({
                "new_binary": c.trap.binary_path.clone(),
                "new_argv": c.trap.argv.clone(),
                "set_env": { "OTEL_TRACE_ID": format!("{:032x}", std::process::id()), "CI": "1" },
                "unset_env": [],
            }))?)))
        }))


        .trap(TrapFilter::binary("git").with_arg("push"), handler(|c: HandlerCtx<_, _>| async move {
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
            move |c: HandlerCtx<_, _>| {
                let oracle = Arc::clone(&oracle);
                async move {
                    let pre: SnapshotDescriptor = c.step.last().ok_or_else(|| {
                        PluginError::Halt("no pre-exec checkpoint".into())
                    })?;
                    let receipt: ExecutionReceipt = c.receipt().await?;


                    // see: cans/10-attestation.md#ExecutionReceipt-records-final-binary-blake3
                    let trap_hex = hex_lower(&c.trap.binary_blake3);
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
            move |c: HandlerCtx<_, _>| async move {


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


        .trap(TrapFilter::binary("git").with_arg("commit"), handler(|c: HandlerCtx<_, _>| async move {
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
        .on_vm_init(|ctx| {
            Box::pin(async move {
                ctx.set_watchdog(Duration::from_secs(5 * 60), kelir::WatchdogAction::Reboot)
                    .await
            })
        })
        // TODO (L3): flush via session.ctx::<LedgerCtx>() once lifecycle hooks receive the typed session per cans/02-plugin-model.md#Trap-and-lifecycle-hooks-receive-HandlerCtx-or-a-restricted-lifecycle-context.
        .on_vm_shutdown(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
        // TODO (L3): count denials via session.ctx::<LedgerCtx>().count_denials(event.ppid) once the deny hook receives the typed session per cans/02-plugin-model.md#Trap-and-lifecycle-hooks-receive-HandlerCtx-or-a-restricted-lifecycle-context.
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
        session.lineage(1),
        SnapshotDescriptor {
            id: "clean_base".into(),
            dirty_bytes: 0,
            created_at_ns: 0,
            parent_digest: None,
            guest_clock: None,
        },
    )?;
    let base = session.checkpoint.commit_step(session.lineage(1))?;
    eprintln!("[kelir] checkpoint base node={base}");

    let phase: Result<(), Box<dyn std::error::Error + Send + Sync>> = async {
        let mut joins = tokio::task::JoinSet::new();
        for id in ['A', 'B', 'C', 'D'] {
            let mut child = session.fork()?;
            joins.spawn(async move {
                child.checkpoint.create_step(
                    child.lineage(1),
                    SnapshotDescriptor {
                        id: format!("hypothesis-{id}"),
                        dirty_bytes: 0,
                        created_at_ns: 0,
                        parent_digest: None,
                        guest_clock: None,
                    },
                )?;
                child.checkpoint.commit_step(child.lineage(1))?;
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
            let lineage = session.lineage(1);
            session.rollback(lineage).await?;
            return Ok(());
        };

        session.suspend(SuspendReason::External(
            ExternalSuspendKind::HumanAuthorizationHold,
        ))?;
        eprintln!("[kelir] suspended: host RSS = 0 MB; awaiting approval");
        oracle.await_human_approval().await?;
        session.resume()?;

        let stats = session.storage.reconcile(FsReconcileRequest {
            cwd: "/workspace/checkout-service".into(),
            atomic_staging: true,
            fsync: true,
        })?;
        eprintln!(
            "[kelir] broker commit files={} deletes={} winner={id}",
            stats.files_changed, stats.tombstones_resolved,
        );
        // TODO (L3): spawn npm test -- --runInBand, git commit the winning fix, and finalize the ledger receipt.
        // TODO (L3): FinalizeRequest.include_delta is CausalDelta, not Option<CausalDelta>; guard the winning delta before finalizing.

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

// see: cans/10-attestation.md#Execution-receipts
pub struct IntentAnalysis {
    pub is_hypothesis_valid: bool,
    pub falsification_reason: Option<String>,
    pub allow_high_entropy: bool,
}

// see: cans/artifacts/code/execution-receipt.rs#ExecutionReceipt
fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[usize::from(*byte >> 4)] as char);
        out.push(HEX[usize::from(*byte & 0x0f)] as char);
    }
    out
}

struct UserOracle {
    state: Mutex<OracleState>,
}

#[derive(Default)]
struct OracleState {
    falsified: Option<String>,
}

impl UserOracle {
    fn new() -> Self {
        Self {
            state: Mutex::new(OracleState::default()),
        }
    }

    async fn analyze_intent(
        &self,
        trap: &TrapEvent,
    ) -> Result<IntentAnalysis, Box<dyn std::error::Error + Send + Sync>> {
        let state = self.state.lock().await;
        if let Some(reason) = state.falsified.clone() {
            return Ok(IntentAnalysis {
                is_hypothesis_valid: false,
                falsification_reason: Some(reason),
                allow_high_entropy: false,
            });
        }
        if trap.argv.is_empty() {
            return Ok(IntentAnalysis {
                is_hypothesis_valid: false,
                falsification_reason: Some("trap argv is empty; no hypothesis to validate".into()),
                allow_high_entropy: false,
            });
        }
        Ok(IntentAnalysis {
            is_hypothesis_valid: true,
            falsification_reason: None,
            allow_high_entropy: false,
        })
    }

    async fn verify_physical_receipt(
        &self,
        receipt: &ExecutionReceipt,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let state = self.state.lock().await;
        if state.falsified.is_some() {
            return Ok(false);
        }
        // see: cans/artifacts/code/execution-receipt.rs#ExecutionReceipt
        Ok(!receipt.binary_blake3.is_empty() && receipt.syscall_count.is_some())
    }

    // see: cans/02-plugin-model.md#Shape-of-the-surface
    fn ingest_harness_event(
        &self,
        path: &Path,
        event: &serde_json::Value,
        _blake3: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut state = self.state.blocking_lock();
        if event
            .get("falsifiedResult")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            state.falsified = Some(format!(
                "harness event {} reported falsifiedResult",
                path.display()
            ));
        }
        Ok(())
    }

    async fn await_human_approval(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.state.lock().await.falsified.is_some() {
            return Err("human approval denied: harness falsification on record".into());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(())
    }
}
