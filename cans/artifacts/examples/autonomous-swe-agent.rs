#![allow(dead_code)]

// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
// see: cans/plugins/ledger.md#Plugin-ledger
// see: cans/plugins/chaos.md#Plugin-chaos

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kelir::async_trait;
use kelir::{Deserialize, Serialize};
use kelir_interface::driver_interface::{ExternalSuspendKind, SuspendReason};
use kelir_interface::plugin_model::HandlerCtx as IfaceHandlerCtx;
use kelir::{CredentialBrokerCtx, VaultPlugin};

use kelir::{
    interface::driver_interface::{DriverPolicy, WxPolicy},
    planes::{
        storage_plane::{
            IdmapConfig, LockBrokerConfig, LockScope, SharedCacheConfig,
            StoragePlaneConfig, WorkspaceEncapsulation,
        },
        terminal_plane::{PtyConfig, TerminalSanitizerConfig, TerminalWireframeGridConfig},
        wire_plane::{L4Proxy, MtlsConfig, SinkholeConfig, SinkholeRule, TapFilter, WireConfig},
    },
    plugin_model::{
        AnyPlugin, FilterAction, FsInterposer, KelirPlugin, PluginDeps, PluginError, TrapInterposer,
    },
    protocol_wire::{Errno, ExecveMutation, PatchPayload, TrapEvent},
    telemetry_attest::ExecutionReceipt,
    ChaosConfig, ChaosPlugin, FaultDirection, FaultInjectionService, FinalizeRequest,
    FsReconcileRequest, FsScopeConfig, HCons, HNil, HandlerCtx, Host, KelirBuilder,
    LedgerConfig, LedgerCtx, LedgerHandle, LedgerPlugin, NetworkFaultRule, NoParams, OtelConfig,
    PostExecAction,
    ReceiptConfig, SnapshotDescriptor, StorageFaultRule, TrapFilter, Unsealed, Verdict, VmConfig,
};

// see: cans/03-sdk-architecture.md#.params(model:-type[P])-pins-Params;-handler-c.params-is-typed
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "kebab-case", crate = "kelir::serde")]
pub enum SpawnAction {
    FixStripeWebhook {
        repo: String,
        issue_id: String,
        env: Env,
    },
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase", crate = "kelir::serde")]
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
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .violations
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
                errno: Errno::Eperm,
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
                errno: Errno::Enospc,
                reason: "diff exceeds policy ceiling".into(),
            });
        }
        Ok(FilterAction::Forward)
    }

    fn on_fs_unlink(&self, path: &Path, _ctx: &IfaceHandlerCtx) -> Result<Verdict, PluginError> {
        // Prevents agents from deleting validation suites or package manifests to fake green runs
        if path.ends_with("package.json")
            || path.ends_with("Cargo.toml")
            || path.to_string_lossy().contains("test")
        {
            if let Some(name) = path.file_name() {
                let quarantined = Path::new("/workspace/.agentsh_trash").join(name);
                return Ok(Verdict::Patch(PatchPayload::FsPathRedirect {
                    target_path: quarantined.to_string_lossy().to_string(),
                }));
            }
        }
        Ok(Verdict::Pass)
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
        HCons<
            RepoPolicyHandle,
            HCons<LedgerCtx, HCons<CredentialBrokerCtx, HCons<Arc<dyn FaultInjectionService>, HNil>>>,
        >,
        NoParams,
    >,
    Box<dyn std::error::Error + Send + Sync>,
> {
    let policy_plugin = RepoPolicyPlugin::new(RepoPolicyOptions {
        max_diff_bytes: 64 * 1024,
        protected_branches: vec!["main".into(), "master".into(), "release/*".into()],
    });
    let policy_handle = policy_plugin.handle();
    let host_uid = {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata("/proc/self")
            .map(|m| m.uid())
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
    };

    let builder = KelirBuilder::new(VmConfig {
        tenant_id: None,
        vcpus: 4,
        memory_mb: 4096,
        kernel_path: format!("./fixtures/vmlinux-6.6-{}-kelir", std::env::consts::ARCH).into(),
        rootfs_erofs_path: "./fixtures/rootfs-swebench.erofs".into(),
        tsc_locked: true,
        enable_watchdog: true,
        enable_gpu_passthrough: false,
        driver_policy: DriverPolicy {
            emulated_tier_classification: None,
            driver: None,
            require_hardware_vmm: false,
            allow_emulated_fallback: true,
            verify_hw_lockfile: false,
            wx_enforcement: WxPolicy::PermissiveAudit,
            ffi_worker_threads: 2,
            ffi_max_ops_per_sec: 1000,
            ffi_queue_capacity: 16384,
            ffi_max_buffer_bytes: 16777216,
            ffi_zero_copy_shm_threshold_bytes: 65536,
            ffi_allow_async_worker_yield: true,
        },
        page_size_kb: 4,
        deterministic_clock: false,
        network_off: false,
        pinned_clocksource: None,
        ns_per_instruction: None,
        preemption_grace_ms: 0,
        entropy_seed: None,
        balloon: None,
        mask_proc_environ: true,
    })
    .terminal(PtyConfig {
        cols: 120,
        rows: 40,
        wireframe_grid: TerminalWireframeGridConfig {
            track_screen_matrix: false,
            export_on_trap: true,
        },
        stream_engine: "StatelessRegex".into(),
        sanitizer: TerminalSanitizerConfig {
            drop_osc52: true,
            strip_title: true,
        },
    })
    .storage(StoragePlaneConfig {
        idmap: IdmapConfig {
            guest_uid: 0,
            host_uid,
        },
        shared_caches: vec![
            SharedCacheConfig {
                host_path: "~/.npm".into(),
                guest_mount: "~/.npm".into(),
                read_only: true,
            },
            SharedCacheConfig {
                host_path: "~/.cache/pip".into(),
                guest_mount: "~/.cache/pip".into(),
                read_only: true,
            },
            SharedCacheConfig {
                host_path: "~/.cargo/registry".into(),
                guest_mount: "~/.cargo/registry".into(),
                read_only: true,
            },
        ],
        encapsulation: WorkspaceEncapsulation::Passthrough,
        dax_window: None,
    })
    .fs(|f| {
        f.config(FsScopeConfig {
            lock_broker: Some(LockBrokerConfig {
                scope: LockScope::Globs(vec![
                    "**/Cargo.lock".into(),
                    "**/package-lock.json".into(),
                ]),
                non_blocking: true,
            }),
        })
        .end()
    })
    .wire(WireConfig {
        sinkhole: SinkholeConfig {
            rules: vec![
                SinkholeRule {
                    host: "api.stripe.com".into(),
                    allow: true,
                },
                SinkholeRule {
                    host: "github.com".into(),
                    allow: true,
                },
                SinkholeRule {
                    host: "*.internal.net".into(),
                    allow: true,
                },
                SinkholeRule {
                    host: "*".into(),
                    allow: false,
                },
            ],
        },
        ssh_agent_bridge: true,

        l4_proxies: vec![L4Proxy {
            listen_port: 5432,
            destination: "pg.internal.net:5432".into(),
            forward_unix_socket: None,
            inject_mtls: Some(MtlsConfig {
                cert_path: pg_cert,
                key_path: pg_key,
            }),
        }],

        tap_filter: TapFilter::AllowUdpProxied,
        userspace_netstack: true,
        default_egress: "drop".into(),
        dial_timeout_ms: 3_000,
    })
    .plugin_instance(ChaosPlugin::new(ChaosConfig {
        network: vec![
            NetworkFaultRule {
                target: "api.stripe.com".into(),
                direction: FaultDirection::Outbound,
                latency_ms: 150,
                jitter_ms: 30,
                drop_rate: 0.02,
                inject_rst: false,
                bandwidth_kbps: None,
            },
            NetworkFaultRule {
                target: "api.stripe.com".into(),
                direction: FaultDirection::Inbound,
                latency_ms: 150,
                jitter_ms: 30,
                drop_rate: 0.02,
                inject_rst: false,
                bandwidth_kbps: None,
            },
            NetworkFaultRule {
                target: "api.stripe.com".into(),
                direction: FaultDirection::Both,
                latency_ms: 150,
                jitter_ms: 30,
                drop_rate: 0.02,
                inject_rst: false,
                bandwidth_kbps: None,
            },
        ],
        storage: vec![StorageFaultRule {
            target_glob: "/workspace/build/cache/*".into(),
            simulate_errno: Errno::Enospc,
            probability: 0.05,
        }],
    }))
    // see: cans/plugins/ledger.md#Plugin-ledger
    .plugin_instance(VaultPlugin::new())
    .plugin_instance(LedgerPlugin::new(true).with_config(LedgerConfig {
        strict: true,
        ocsf_mapping: true,
        hashing_standard: "RFC_6962_DUAL_BLAKE3_SHA256".into(),
        record_pre_redaction_hash: false,
        include_hook_spans_in_root: false,
        witness_cosigning_endpoint: None,
        store_path: ".kelir/audit.merkle.db".into(),
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
    .trap_async("*", {
        let oracle = Arc::clone(&oracle);
        move |c: HandlerCtx<_, _>| {
            let oracle = Arc::clone(&oracle);
            async move {
                let _pre = c.vm.checkpoint(format!("pre:{}", c.trap.pid)).await?;

                let intent: IntentAnalysis = oracle
                    .analyze_intent(&c.trap)
                    .await
                    .map_err(|err| PluginError::Halt(format!("intent analysis failed: {err}")))?;
                if !intent.is_hypothesis_valid {
                    return Ok(Verdict::Drop {
                        errno: Errno::Eperm,
                        reason: format!(
                            "Verification Reject: {}",
                            intent.falsification_reason.unwrap_or_default()
                        ),
                    });
                }
                Ok(Verdict::Pass)
            }
        }
    })
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
    .post_exec("*", {
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
                        PluginError::Halt(format!("physical receipt verification failed: {err}"))
                    })?
                {
                    c.vm.rollback(&pre).await?;
                    c.terminal
                        .notice("⚡ [KELIR]: post-condition violated — REVERTED\n");
                    return Ok(PostExecAction::RollbackAndHalt);
                }
                Ok(PostExecAction::Proceed)
            }
        }
    })
    .trap_async(
        TrapFilter::binary("git").with_arg("commit").with_arg("-m"),
        {
            let policy = policy_handle.clone();
            move |_c: HandlerCtx<_, _>| {
                let policy = policy.clone();
                async move {
                    let n = policy.violations().await;
                    if n > 0 {
                        return Ok(Verdict::Drop {
                            errno: Errno::Eperm,
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
                    "[kelir] commit fs:vfs extension present, served_inodes={} roots={}",
                    ext.served_inodes,
                    ext.roots.len(),
                ),
                None => eprintln!("[kelir] commit fs:vfs extension absent"),
            }
            Ok(PostExecAction::Proceed)
        },
    )
    .lifecycle(|l| {
        l.on_vm_init(|ctx| {
            Box::pin(async move {
                ctx.set_watchdog(Duration::from_secs(5 * 60), kelir::WatchdogAction::Reboot)
                    .await
            })
        })
        .on_vm_shutdown(|ctx| {
            Box::pin(async move {
                if let Some(ledger) = ctx.cap::<LedgerHandle>() {
                    ledger.flush().await?;
                }
                Ok(())
            })
        })
        .end()
    })
    .deny(|d| {
        d.on_trap_deny(|_event, _verdict, _session| async move { Ok::<(), PluginError>(()) })
            .end()
    });

    Ok(builder)
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    kelir::block_on(async_main())?
}

async fn async_main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

    session.step_evaluator().begin_step(
        session.lineage(1),
        SnapshotDescriptor {
            id: "clean_base".into(),
            dirty_bytes: 0,
            created_at_ns: 0,
            parent_digest: None,
            guest_clock: None,
        },
    )?;
    let base = session.step_evaluator().commit_step(session.lineage(1))?;
    eprintln!("[kelir] checkpoint base node={base}");

    let phase: Result<(), Box<dyn std::error::Error + Send + Sync>> = async {
        let mut joins = kelir::JoinSet::new();
        for id in ['A', 'B', 'C', 'D'] {
            let mut child = session.fork().await?;
            joins.spawn(async move {
                let outcome: Result<kelir::CausalDelta, Box<dyn std::error::Error + Send + Sync>> =
                    async {
                        child
                            .vm
                            .clock
                            .skew(Duration::from_secs(
                                86400 * ((id as u64) - ('A' as u64) + 1),
                            ))
                            .await?;
                        child.step_evaluator().begin_step(
                            child.lineage(1),
                            SnapshotDescriptor {
                                id: format!("hypothesis-{id}"),
                                dirty_bytes: 0,
                                created_at_ns: 0,
                                parent_digest: None,
                                guest_clock: None,
                            },
                        )?;
                        child.step_evaluator().commit_step(child.lineage(1))?;
                        let run = child
                            .spawn("npm", &["run", &format!("verify-hypothesis-{id}")], None)
                            .await?;
                        if run.exit_code != 0 {
                            return Err(format!(
                                "hypothesis {id} verification failed: code {}",
                                run.exit_code
                            )
                            .into());
                        }
                        let delta = child
                            .cap::<LedgerHandle>()
                            .ok_or("ledger missing")?
                            .get_causal_delta_from_parent()
                            .await?;
                        child.stop().await?;
                        Ok(delta)
                    }
                    .await;
                (id, outcome)
            });
        }

        let mut exit_codes = Vec::new();
        while let Some(joined) = joins.join_next().await {
            exit_codes.push(
                joined.map_err(|err| Box::new(err) as Box<dyn std::error::Error + Send + Sync>)?,
            );
        }

        let mut winner_info = None;
        for branch_id in ['A', 'B', 'C', 'D'] {
            if let Some((_, Ok(delta))) = exit_codes.iter().find(|(branch, res)| *branch == branch_id && res.is_ok()) {
                winner_info = Some((branch_id, delta.clone()));
                break;
            }
        }

        let Some((id, winning_delta)) = winner_info else {
            let lineage = session.lineage(1);
            session.rollback(lineage).await?;
            return Ok(());
        };

        session
            .suspend(SuspendReason::External(
                ExternalSuspendKind::External("human-authorization-hold".into()),
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

        let ledger = session
            .cap::<LedgerHandle>()
            .ok_or("ledger capability missing")?;
        let final_receipt = ledger
            .finalize(FinalizeRequest {
                include_delta: Some(winning_delta),
                include_merkle_root: true,
            })
            .await?;
        eprintln!(
            "[kelir] receipt merkle={} jws={}…",
            final_receipt.merkle_root,
            &final_receipt.signed_jws[..64]
        );

        Ok(())
    }
    .await;

    if let Err(e) = session
        .suspend(SuspendReason::External(
            ExternalSuspendKind::External("human-authorization-hold".into()),
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
        event: &kelir::serde_json::Value,
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
        kelir::time::sleep(Duration::from_millis(50)).await;
        Ok(())
    }
}
