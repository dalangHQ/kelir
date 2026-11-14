#![allow(dead_code)]

// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
// see: cans/plugins/ledger.md#Plugin-ledger
// see: cans/plugins/chaos.md#Plugin-chaos
// L3-GAP: mTLS injection at the L4-proxy host boundary; ledger denial recording
//   and ledger flush in trap/lifecycle/deny hooks; child clock skew plus
//   verify-hypothesis spawn and causal-delta read; winning-delta finalize
//   (FinalizeRequest.include_delta guard) — all blocked on typed session
//   access in those hooks and LedgerCtx surface growth.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use kelir_interface::credential_vault::{CredentialDescriptor, CredentialKind};
use kelir_interface::driver_interface::{ExternalSuspendKind, SuspendReason};
use kelir_interface::plugin_model::HandlerCtx as IfaceHandlerCtx;
use serde::{Deserialize, Serialize};

use kelir::{
    ChaosConfig, ChaosPlugin, FaultDirection, FaultInjectionService, FsReconcileRequest, HCons,
    HNil, HandlerCtx, Host, KelirBuilder, LedgerConfig, LedgerCtx, LedgerPlugin,
    NetworkFaultRule, NoParams, OtelConfig, PostExecAction, ReceiptConfig, SnapshotDescriptor,
    StorageFaultRule, TrapFilter, Unsealed, Verdict, VmConfig,
    interface::driver_interface::{DriverPolicy, WxPolicy},
    planes::{
        storage_plane::{
            DynamicFile, IdmapConfig, MaskReadRule, ReadMaskEngine, SnoopRule,
            StoragePlaneConfig, SyntheticFile, WorkspaceEncapsulation,
        },
        terminal_plane::{PromptRule, PtyConfig, TokenShieldConfig},
        wire_plane::{L4Proxy, SinkholeConfig, TapFilter, WireConfig},
    },
    plugin_model::{
        AnyPlugin, FilterAction, FsInterposer, KelirPlugin, PluginDeps, PluginError,
        TrapInterposer,
    },
    protocol_wire::{Errno, ExecveMutation, PatchPayload, TrapEvent},
    telemetry_attest::ExecutionReceipt,
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
        self.state.lock().unwrap_or_else(|e| e.into_inner()).violations
    }
    pub async fn branch(&self) -> String {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .current_branch
            .clone()
    }
    pub async fn record_violation(&self) {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .violations += 1;
    }
    pub async fn set_branch(&self, b: impl Into<String>) {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .current_branch = b.into();
    }
    pub async fn is_protected(&self) -> bool {
        let s = self.state.lock().unwrap_or_else(|e| e.into_inner());
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
            state: Arc::new(Mutex::new(
                self.state.lock().unwrap_or_else(|e| e.into_inner()).clone(),
            )),
        })
    }
}

impl PluginDeps for RepoPolicyPlugin {
    type Deps = HCons<LedgerCtx, HNil>;
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
                errno: Some(Errno::Eperm),
                reason: format!("EPOLICY: '{}' is protected.", h.branch().await),
            });
        }

        let new_branch = format!("agent/{:08x}", std::process::id());
        let mut argv = event.argv.clone();
        argv.push(format!("HEAD:refs/heads/{new_branch}"));
        // ExecveMutation (plugins/tool-guard.md) as the typed Patch payload;
        // serializes to CBOR on the wire (11-protocol.md#Wire-framing).
        let mutation = ExecveMutation {
            new_binary: Some("/bin/git".into()),
            new_argv: Some(argv),
            set_env: vec![("KELIR_REWRITTEN".into(), "1".into())],
            unset_env: vec![],
        };
        Ok(Verdict::Patch(PatchPayload::ExecveMutation(mutation)))
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
            self.state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .violations += 1;
            return Ok(FilterAction::Drop {
                errno: Some(Errno::Enospc),
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
        Host,
        Unsealed,
        (),
        HNil,
        HCons<RepoPolicyHandle, HCons<LedgerCtx, HCons<Arc<dyn FaultInjectionService>, HNil>>>,
        NoParams,
    >,
    Box<dyn std::error::Error + Send + Sync>,
> {
    let policy_plugin = RepoPolicyPlugin::new(RepoPolicyOptions {
        max_diff_bytes: 64 * 1024,
        protected_branches: vec!["main".into(), "master".into(), "release/*".into()],
    });
    let policy_handle = policy_plugin.handle();

    let builder = KelirBuilder::new(VmConfig {
        vcpus: 4,
        memory_mb: 4096,
        kernel_path: format!("./fixtures/vmlinux-6.6-{}-kelir", std::env::consts::ARCH).into(),
        rootfs_erofs_path: "./fixtures/rootfs-swebench.erofs".into(),
        tsc_locked: true,
        enable_watchdog: true,
        enable_gpu_passthrough: false,
        driver_policy: DriverPolicy {
            driver: None,
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
        token_shield: Some(TokenShieldConfig {
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
    .storage(StoragePlaneConfig {
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
        mask_read: vec![MaskReadRule {
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
            path: "~/.claude/sessions/*.json".into(),
            sync: true,
            on_write: Arc::new({
                let oracle = Arc::clone(&oracle);
                move |path: &Path, raw: &[u8], _ctx: &IfaceHandlerCtx| {
                    let oracle = Arc::clone(&oracle);
                    let path = path.to_path_buf();
                    let raw = raw.to_vec();
                    Box::pin(async move {
                        let evt: serde_json::Value = serde_json::from_slice(&raw).map_err(|err| {
                            PluginError::Halt(format!("snoop payload decode failed: {err}"))
                        })?;
                        if evt
                            .get("falsifiedResult")
                            .and_then(|value| value.as_bool())
                            .unwrap_or(false)
                        {
                            return Err(PluginError::reject(
                                Errno::Eperm,
                                format!("harness hallucination at {}", path.display()),
                            ));
                        }
                        // TODO (L3): resolve blake3_of_producing_process once the snoop ctx carries the ledger cap per cans/plugins/ledger.md#Ledger-handler-API.
                        oracle
                            .ingest_harness_event(&path, &evt, "")
                            .await
                            .map_err(|err| PluginError::Halt(err.to_string()))
                    })
                }
            }),
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
        }],

        tap_filter: TapFilter::AllowUdpProxied,
    })
    .plugin_instance(ChaosPlugin::new(ChaosConfig {
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
            simulate_error: Errno::Enospc,
            error_probability: 0.05,
        }],
    }))
    // see: cans/plugins/ledger.md#Plugin-ledger
    .plugin_instance(LedgerPlugin::new(true).with_config(LedgerConfig {
        store: ".kelir/audit.merkle.db".into(),
        merkle_chain: true,
        suppress_local_db: false,
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
    }))
    .plugin_instance(policy_plugin)
    .trap_async(
            "*",
            {
                let oracle = Arc::clone(&oracle);
                move |c: HandlerCtx<_, _>| {
                    let oracle = Arc::clone(&oracle);
                    async move {
                        let _pre = c.vm.checkpoint(format!("pre:{}", c.trap.pid)).await?;

                        let intent: IntentAnalysis = oracle
                            .analyze_intent(&c.trap)
                            .await
                            .map_err(|err| {
                                PluginError::Halt(format!("intent analysis failed: {err}"))
                            })?;
                        if !intent.is_hypothesis_valid {
                            return Ok(Verdict::Drop {
                                errno: Some(Errno::Eperm),
                                reason: format!(
                                    "Verification Reject: {}",
                                    intent.falsification_reason.unwrap_or_default()
                                ),
                            });
                        }
                        Ok(Verdict::Pass)
                    }
                }
            },
        )
        .trap_async(
            TrapFilter::binary("npm")
                .with_arg("test")
                .with_arg("--")
                .with_arg("--runInBand"),
            |c: HandlerCtx<_, _>| async move {
                c.fs.warm_cache(&["node_modules", ".next/cache"]).await?;
                // Env injection via Patch, not env.set + Pass (09: EINVALIDDECISION).
                // SDK patch carries the typed ExecveMutation (CBOR on the wire).
                Ok(Verdict::Patch(PatchPayload::ExecveMutation(
                    ExecveMutation {
                        new_binary: Some(c.trap.binary_path.clone()),
                        new_argv: Some(c.trap.argv.clone()),
                        set_env: vec![
                            (
                                "OTEL_TRACE_ID".into(),
                                format!("{:032x}", std::process::id()),
                            ),
                            ("CI".into(), "1".into()),
                        ],
                        unset_env: vec![],
                    },
                )))
            },
        )
        .trap_async(
            TrapFilter::binary("git").with_arg("push"),
            |c: HandlerCtx<_, _>| async move {
                if c.trap.argv.iter().any(|a| a == "--force") {
                    let argv: Vec<String> = c
                        .trap
                        .argv
                        .iter()
                        .filter(|a| *a != "--force")
                        .cloned()
                        .collect();
                    return Ok(Verdict::Patch(PatchPayload::ExecveMutation(
                        ExecveMutation {
                            new_binary: Some("/bin/git".into()),
                            new_argv: Some(argv),
                            set_env: vec![("KELIR_DISARMED".into(), "1".into())],
                            unset_env: vec![],
                        },
                    )));
                }
                Ok(Verdict::Pass)
            },
        )
        .post_exec(
            "*",
            {
                let oracle = Arc::clone(&oracle);
                move |c: HandlerCtx<_, _>| {
                    let oracle = Arc::clone(&oracle);
                    async move {
                        let pre: SnapshotDescriptor = c
                            .step
                            .last()
                            .ok_or_else(|| PluginError::Halt("no pre-exec checkpoint".into()))?;
                        let receipt: ExecutionReceipt = c.receipt().await?;

                        // see: cans/10-attestation.md#binary_blake3-identity
                        let trap_hex = hex_lower(&c.trap.binary_blake3);
                        if receipt.binary_blake3 != trap_hex {
                            c.vm.rollback(&pre).await?;
                            c.terminal
                                .notice("⚡ [KELIR]: post-exec binary hash mismatch — REVERTED\n");
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

                        if !oracle
                            .verify_physical_receipt(&receipt)
                            .await
                            .map_err(|err| {
                                PluginError::Halt(format!(
                                    "physical receipt verification failed: {err}"
                                ))
                            })? {
                            c.vm.rollback(&pre).await?;
                            c.terminal
                                .notice("⚡ [KELIR]: post-condition violated — REVERTED\n");
                            return Ok(PostExecAction::RollbackAndHalt);
                        }
                        Ok(PostExecAction::Proceed)
                    }
                }
            },
        )
        .trap_async(
            TrapFilter::binary("git").with_arg("commit").with_arg("-m"),
            {
                let policy = policy_handle.clone();
                move |c: HandlerCtx<_, _>| {
                    let policy = policy.clone();
                    async move {
                        let n = policy.violations().await;
                        if n > 0 {
                            return Ok(Verdict::Drop {
                                errno: Some(Errno::Eperm),
                                reason: format!("EPOLICY: {n} prior violation(s)."),
                            });
                        }
                        Ok(Verdict::Pass)
                    }
                }
            },
        )
        .post_exec(
            TrapFilter::binary("git").with_arg("commit"),
            |c: HandlerCtx<_, _>| async move {
                let receipt = c.receipt().await?;

                if receipt.dirty_bytes == 0 {
                    return Err(PluginError::Halt("zero-byte delta commit".into()));
                }

                // see: cans/10-attestation.md#Typed-extensions
                match receipt.fs_vfs() {
                    Some(ext) => eprintln!(
                        "[kelir] commit fs:vfs extension present, payload_len={}",
                        ext.payload.len(),
                    ),
                    None => eprintln!("[kelir] commit fs:vfs extension absent"),
                }
                Ok(PostExecAction::Proceed)
            },
        )
        .on_vm_init(|ctx| {
            Box::pin(async move {
                ctx.set_watchdog(Duration::from_secs(5 * 60), kelir::WatchdogAction::Reboot)
                    .await
            })
        })
        .on_vm_shutdown(|_ctx| Box::pin(async move { Ok::<(), PluginError>(()) }))
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

    let mut session = builder.seal()?.boot().await?;
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
            let mut child = session.fork().await?;
            joins.spawn(async move {
                let outcome: Result<(), Box<dyn std::error::Error + Send + Sync>> = async {
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
                    child.stop().await?;
                    Ok(())
                }
                .await;
                // Winner rule: first branch in branch order with exit code 0 wins.
                (id, i32::from(outcome.is_err()))
            });
        }

        let mut exit_codes = Vec::new();
        while let Some(joined) = joins.join_next().await {
            exit_codes.push(
                joined.map_err(|err| Box::new(err) as Box<dyn std::error::Error + Send + Sync>)?,
            );
        }
        // Winner rule: first branch in branch order with exit code 0 wins.
        let winner = ['A', 'B', 'C', 'D'].into_iter().find(|id| {
            exit_codes
                .iter()
                .any(|(branch, code)| branch == id && *code == 0)
        });
        let Some(id) = winner else {
            let lineage = session.lineage(1);
            session.rollback(lineage).await?;
            return Ok(());
        };

        session.suspend(SuspendReason::External(
            ExternalSuspendKind::HumanAuthorizationHold,
        ))
        .await?;
        eprintln!("[kelir] suspended: host RSS = 0 MB; awaiting approval");
        oracle.await_human_approval().await?;
        session.resume().await?;

        let stats = session.storage.reconcile(FsReconcileRequest {
            cwd: "/workspace/checkout-service".into(),
            atomic_staging: true,
            fsync: true,
        })?;
        eprintln!(
            "[kelir] broker commit files={} deletes={} winner={id}",
            stats.files_changed, stats.tombstones_resolved,
        );

        Ok(())
    }
    .await;

    if let Err(e) = session
        .suspend(SuspendReason::External(
            ExternalSuspendKind::HumanAuthorizationHold,
        ))
        .await
    {
        eprintln!("[kelir] final suspend failed: {e}");
    }
    session.stop().await?;
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
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
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
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.falsified.is_some() {
            return Ok(false);
        }
        // see: cans/artifacts/code/execution-receipt.rs#ExecutionReceipt
        Ok(!receipt.binary_blake3.is_empty() && receipt.syscall_count.is_some())
    }

    // see: cans/02-plugin-model.md#Shape-of-the-surface
    async fn ingest_harness_event(
        &self,
        path: &Path,
        event: &serde_json::Value,
        _blake3: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
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
        if self
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .falsified
            .is_some()
        {
            return Err("human approval denied: harness falsification on record".into());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(())
    }
}
