//! examples/autonomous-swe-agent.rs
//!
//! Full-surface Rust SDK consumer. Not a TypeScript transliteration — this
//! is the intended *native* Rust surface of the `kelir` facade crate
//! (`kelir-core` + first-class plugins), written to be consumed directly
//! (not through the napi/pyo3 bindings, which are for TS/Python consumers).
//!
//! Key runtime guarantees enforced natively:
//!
//!   1. Fluent builder chaining: `.plugin()` returns `Builder` directly;
//!      capabilities project onto `c.policy()` and `session.policy()`,
//!      while pre-boot host access is held via the instantiated handle.
//!
//!   2. `spawn().params::<P>(…)` requires `P: Deserialize`, and the runtime
//!      deserializes the incoming JSON into `P` before any handler fires —
//!      the TypeBox `t.Object({…})` runtime guard expressed as a zero-cost
//!      serde target.
//!
//! Every plugin, every §11 hypervisor primitive, and the §12 three-tier evidence verification model,
//! and all four VirtIO rings are exercised in the phases below.
//!
//! Cargo.toml:
//!   [dependencies]
//!   kelir = { version = "3.0", features = ["pty","vfs","wire","chaos","ledger"] }
//!   tokio = { version = "1", features = ["rt-multi-thread", "macros", "net", "sync", "io-util", "time"] }
//!   async-trait = "0.1"
//!   serde = { version = "1", features = ["derive"] }
//!   serde_json = "1"
//!   thiserror = "2"
//!   blake3 = "1"
//!   libc = "0.2"

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use kelir::{
    handler, Decision, DriverChoice, FinalizeRequest, FsReconcileRequest, HandlerCtx,
    KelirBuilder, PostExecAction, Session, SnapshotDescriptor, StepSlot, VmConfig,
    oracle::{IntentAnalysis, Oracle},
    plugin::{KelirPlugin, PluginError},
    plugins::{
        chaos::{ChaosConfig, ChaosPlugin, FsChaos, NetworkChaos},
        ledger::{FinalReceipt, LedgerConfig, LedgerPlugin, OtelConfig, ReceiptConfig},
        pty::{PtyConfig, PtyPlugin, PromptRule, TokenShield},
        vfs::{
            DynamicFile, SyntheticFile, IdmapConfig, MaskRule, SnoopHandler, SnoopRule,
            VfsConfig, VfsPlugin,
        },
        wire::{L4Proxy, Mtls, SinkholeConfig, VaultEntry, WireConfig, WirePlugin},
    },
    protocol::{CausalDelta, ExecutionReceipt, TrapAction, TrapEvent},
};

// ============================================================================
// 1. TYPED PARAMS — the serde analogue of TypeBox's `.guard({ schema })`
// ============================================================================

/// Deserialization target for every `spawn()` call in this file. Any handler
/// that calls `.params::<SpawnAction>(…)` forces serde to validate the
/// inbound JSON against this shape before dispatch. If a field is missing
/// or mistyped at runtime, the spawn fails with `EINVAL` *before* any hook
/// fires — the TypeBox guarantee, enforced by `serde`.
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
pub enum Env { Staging, Production }

// ============================================================================
// 2. CUSTOM PLUGIN — `KelirPlugin` across 3 of the 4 VirtIO rings
// ============================================================================

#[derive(Debug, Clone)]
pub struct RepoPolicyOptions {
    pub max_diff_bytes: usize,
    pub protected_branches: Vec<String>,
}

#[derive(Debug, Default)]
struct RepoPolicyState {
    violations: usize,
    current_branch: String,
}

/// Typed handle — the compile-time proof of installation (§6's invariant,
/// expressed in Rust). `Arc<Mutex<_>>` gives interior mutability while
/// keeping the handle `Send + Sync + Clone`, so multiple async handler
/// closures can share it across spawns.
#[derive(Clone)]
pub struct RepoPolicyHandle {
    state: Arc<Mutex<RepoPolicyState>>,
    protected: Arc<Vec<String>>,
}

impl RepoPolicyHandle {
    pub async fn violations(&self) -> usize { self.state.lock().await.violations }
    pub async fn branch(&self) -> String { self.state.lock().await.current_branch.clone() }
    pub async fn record_violation(&self) { self.state.lock().await.violations += 1; }
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

/// Taxonomy: urn:kelir:sys:trap:exec
/// Rings: virtio-vsock, virtio-fs
pub struct RepoPolicyPlugin {
    opts: RepoPolicyOptions,
    state: Arc<Mutex<RepoPolicyState>>,
}

impl RepoPolicyPlugin {
    pub fn new(opts: RepoPolicyOptions) -> Self {
        Self { opts, state: Arc::new(Mutex::new(RepoPolicyState::default())) }
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

#[async_trait]
impl KelirPlugin for RepoPolicyPlugin {
    fn name(&self) -> &'static str { "repo-policy" }

    // ── Ring 2 (virtio-vsock): veto or rewrite `git push` ────────────────
    async fn on_trap(&self, event: &TrapEvent) -> Result<Option<TrapAction>, PluginError> {
        let is_git = event.binary_path.ends_with("/git") || event.binary_path == "git";
        let is_push = event.argv.get(1).map(String::as_str) == Some("push");
        if !(is_git && is_push) { return Ok(None); }

        let h = self.handle();
        if h.is_protected().await {
            h.record_violation().await;
            return Ok(Some(TrapAction::Deny {
                exit_code: 1,
                stderr_message: format!("EPOLICY: '{}' is protected.", h.branch().await),
            }));
        }

        let new_branch = format!("agent/{:08x}", std::process::id());
        let mut argv = event.argv.clone();
        argv.push(format!("HEAD:refs/heads/{new_branch}"));
        let mut env = HashMap::new();
        env.insert("KELIR_REWRITTEN".into(), "1".into());
        Ok(Some(TrapAction::Rewrite {
            new_binary: "/bin/git".into(),
            new_argv: argv,
            injected_env: env,
        }))
    }

    // ── Ring 3 (virtio-fs): reject oversize diff writes at FUSE boundary ─
    async fn on_vfs_write(
        &self, path: &Path, data: &[u8],
    ) -> Result<(), PluginError> {
        if path.extension().is_some_and(|e| e == "diff")
            && data.len() > self.opts.max_diff_bytes
        {
            self.handle().record_violation().await;
            return Err(PluginError::Reject {
                errno: "ENOSPC".into(),
                reason: "diff exceeds policy ceiling".into(),
            });
        }
        Ok(())
    }

    /// on_disclosure: append policy trace to every mandatory notice.
    /// Body is immutable; hook can only append.
    async fn on_disclosure(
        &self, body: &[u8],
    ) -> Result<Option<Vec<u8>>, PluginError> {
        let ann = format!("[repo-policy: {} open violations]\n", self.handle().violations().await);
        Ok(Some([body, ann.as_bytes()].concat()))
    }
}

// ============================================================================
// 3. RUNTIME BUILDER — the plugin chain (write-only until `.boot()`)
// ============================================================================

async fn build_runtime(
    oracle: Arc<dyn Oracle>,
    stripe_key: String,
    pg_cert: PathBuf,
    pg_key: PathBuf,
) -> Result<KelirBuilder, Box<dyn std::error::Error + Send + Sync>> {
    let policy_plugin = RepoPolicyPlugin::new(RepoPolicyOptions {
        max_diff_bytes: 64 * 1024,
        protected_branches: vec!["main".into(), "master".into(), "release/*".into()],
    });
    let policy = policy_plugin.handle();

    // Snoop closure — does NOT capture `session`; uses only the `HandlerCtx`
    // it receives, whose `ledger` field is bound by `LedgerPlugin` below.
    // Same invariant as the TS example (§12.4 Tier-3 truth).
    let snoop: SnoopHandler = Arc::new(|path: PathBuf, raw: Vec<u8>, c: HandlerCtx| {
        Box::pin(async move {
            let evt: serde_json::Value = serde_json::from_slice(&raw)
                .map_err(|e| PluginError::Halt(format!("session is not JSON: {e}")))?;
            if evt.get("falsifiedResult").and_then(|v| v.as_bool()).unwrap_or(false) {
                return Err(PluginError::reject("EPERM", "harness hallucination detected"));
            }
            let blake3 = c.ledger.blake3_of_producing_process(&path).await?;
            oracle.ingest_harness_event(&path, &evt, &blake3).await?;
            Ok(())
        })
    });

    // Every plugin contributes exactly one typed handle slot; no two
    // `.plugin_*()` calls touch the same downstream surface.
    let builder = KelirBuilder::new(VmConfig {
        vcpus: 4,
        memory_mb: 4096,
        kernel: format!("./fixtures/vmlinux-6.6-{}-kelir", std::env::consts::ARCH).into(),
        rootfs_erofs: "./fixtures/rootfs-swebench.erofs".into(),
        tsc_locked: true,
        enable_watchdog: true,
        enable_gpu_passthrough: false,
        driver: DriverChoice::Auto,
        entropy_seed: None,
    })

        // ── TERMINAL PLANE (§7) ────────────────────────────────────────
        .plugin(PtyPlugin::new(PtyConfig {
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
                action: Arc::new(|_, stream| Box::pin(async move {
                    stream.write(b"y\n").await?;
                    Ok(())
                })),
            }],
        }))

        // ── STORAGE PLANE (§8) ─────────────────────────────────────────
        .plugin(VfsPlugin::new(VfsConfig {
            idmap: IdmapConfig {
                guest_uid: 0,
                host_uid: unsafe { libc::getuid() },
            },
            shared_caches: vec![
                "~/.npm".into(), "~/.cache/pip".into(), "~/.cargo/registry".into(),
            ],
            lock_broker: true,
            synthetic_files: vec![
                SyntheticFile::static_(
                    "/workspace/.env.local",
                    "DATABASE_URL=postgres://app:vault@127.0.0.1:5432/prod\n",
                ),
                SyntheticFile::dynamic("/workspace/identity.jwt", |_| Box::pin(async move {
                    Ok(DynamicFile {
                        content: format!("Bearer {:032x}", std::process::id()),
                        mode: 0o400,
                    })
                })),
            ],
            mask_read: vec![MaskRule {
                glob: "**/secrets/**".into(),
                patterns: vec![
                    (r"sk_live_[0-9a-zA-Z]{24}".into(), "[STRIPE_VAULT_KEY]".into()),
                    (r"AKIA[0-9A-Z]{16}".into(), "[AWS_IAM_REDACTED]".into()),
                ],
            }],
            snoops: vec![SnoopRule {
                glob: "~/.claude/sessions/*.json".into(),
                sync: true,
                on_write: snoop,
            }],
        }))

        // ── WIRE PLANE (§9) ─────────────────────────────────────────────
        .plugin(WirePlugin::new(WireConfig {
            sinkhole: SinkholeConfig {
                allowlist: vec![
                    "api.stripe.com".into(), "github.com".into(), "*.internal.net".into(),
                ],
                drop_all: true,
            },
            ssh_agent_bridge: true,
            vaults: vec![VaultEntry {
                name: "STRIPE_SECRET".into(),
                placeholder: "vault://stripe_key".into(),
                resolve: Arc::new(move || {
                    let k = stripe_key.clone();
                    Box::pin(async move { Ok(k) })
                }),
                scope_urls: vec!["https://api.stripe.com/v1/*".into()],
                scrub_response_headers: vec!["set-cookie".into(), "request-id".into()],
            }],
            l4_proxies: vec![L4Proxy {
                listen_port: 5432,
                destination: "pg.internal.net:5432".into(),
                inject_mtls: Some(Mtls { cert_path: pg_cert, key_path: pg_key }),
            }],
        }))

        // ── CHAOS (§10) ─────────────────────────────────────────────────
        .plugin(ChaosPlugin::new(ChaosConfig {
            network: vec![NetworkChaos {
                target: "api.stripe.com".into(),
                base_ms: 150, jitter_ms: 30, drop_rate: 0.02,
                inject_rst: false, bandwidth_kbps: None,
            }],
            fs: vec![FsChaos {
                target_path: "/workspace/build/cache/*".into(),
                simulate_error: "ENOSPC".into(),
                error_probability: 0.05,
            }],
        }))

        // ── LEDGER (§12.1–§12.3) ────────────────────────────────────────
        .plugin(LedgerPlugin::new(LedgerConfig {
            // The audit DB lives on the host, immune to guest rollback/fork.
            store: ".kelir/audit.merkle.db".into(),
            merkle_chain: true,
            otel: Some(OtelConfig {
                endpoint: "http://localhost:4318/v1/traces".into(),
                service_name: "kelir-swe-agent".into(),
                propagate_traceparent: true,
            }),
            receipts: ReceiptConfig {
                include_syscall_count: true,
            },
        }));

    // ── CUSTOM PLUGIN: fluid registration ───────────────────────────────
    let builder = builder.plugin(policy_plugin);

    // ── EXEC GRAPH (§4.3, §12.5) ────────────────────────────────────────
    let builder = builder.exec({
        let oracle = Arc::clone(&oracle);
        move |exec| {
            let policy_for_commit = policy.clone();
            exec
                // Universal atomic step loop: checkpoint → oracle → resume
                .before("*", handler({
                    let oracle = Arc::clone(&oracle);
                    move |c: HandlerCtx| {
                        let oracle = Arc::clone(&oracle);
                        async move {
                            let pre = c.vm
                                .checkpoint(format!("pre:{}", c.trap.pid))
                                .await?;

                            let intent: IntentAnalysis = oracle.analyze_intent(&c.trap).await?;
                            if !intent.is_hypothesis_valid {
                                return Ok(Decision::deny(
                                    1,
                                    format!(
                                        "Verification Reject: {}",
                                        intent.falsification_reason.unwrap_or_default()
                                    ),
                                ));
                            }
                            if c.trap.entropy > 5.2 && !intent.allow_high_entropy {
                                return Ok(Decision::deny(
                                    126,
                                    "EENTROPY: high-entropy argv requires oracle clearance.".into(),
                                ));
                            }
                            Ok(Decision::Resume)
                        }
                    }
                }))

                // Warm caches + stamp traceparent for hermetic test
                .before("npm test -- --runInBand", handler(|c: HandlerCtx| async move {
                    c.env.set("OTEL_TRACE_ID", format!("{:032x}", std::process::id()));
                    c.env.set("CI", "1");
                    c.fs.warm_cache(&["node_modules", ".next/cache"]).await?;
                    Ok(Decision::Resume)
                }))

                // Entropy defense + `--force` strip
                .before("git push *", handler(|c: HandlerCtx| async move {
                    if c.trap.entropy > 5.2 {
                        return Ok(Decision::deny(1, "EPERM: high-entropy payload.".into()));
                    }
                    if c.trap.argv.iter().any(|a| a == "--force") {
                        let argv = c.trap.argv.iter()
                            .filter(|a| *a != "--force").cloned().collect();
                        return Ok(Decision::rewrite(
                            "/bin/git",
                            argv,
                            [("KELIR_DISARMED".into(), "1".into())],
                        ));
                    }
                    Ok(Decision::Resume)
                }))

                // Tier-1 post-condition verification + rollback (§12.5)
                .after("*", handler({
                    let oracle = Arc::clone(&oracle);
                    move |c: HandlerCtx| {
                        let oracle = Arc::clone(&oracle);
                        async move {
                            let pre: SnapshotDescriptor = c.step.last().ok_or_else(|| {
                                PluginError::Halt("no pre-exec checkpoint".into())
                            })?;
                            let receipt: ExecutionReceipt = c.receipt().await?;

                            // Binary hash integrity — the agent cannot fake this
                            if receipt.binary_blake3 != c.trap.binary_blake3 {
                                c.vm.rollback(&pre).await?;
                                c.term.notice(
                                    "⚡ [KELIR]: post-exec binary hash mismatch — REVERTED\n",
                                );
                                return Ok(PostExecAction::RollbackAndHalt);
                            }
                            // Hermetic runs must not egress
                            if c.trap.binary_path.ends_with("/npm")
                                && receipt.egress_bytes > 0
                                && c.trap.cwd.contains("/checkout-service")
                            {
                                c.vm.rollback(&pre).await?;
                                c.term.notice(&format!(
                                    "⚡ [KELIR]: hermetic test emitted {}B egress — REVERTED\n",
                                    receipt.egress_bytes
                                ));
                                return Ok(PostExecAction::RollbackAndHalt);
                            }
                            // Oracle post-condition (§12.5)
                            if !oracle.verify_physical_receipt(&receipt).await? {
                                c.vm.rollback(&pre).await?;
                                c.term.notice(
                                    "⚡ [KELIR]: post-condition violated — REVERTED\n",
                                );
                                return Ok(PostExecAction::RollbackAndHalt);
                            }
                            Ok(PostExecAction::Proceed)
                        }
                    }
                }))

                // Commit gate — refuses if the policy plugin logged anything
                .before("git commit -m *", handler({
                    let policy = policy_for_commit.clone();
                    move |_c: HandlerCtx| {
                        let policy = policy.clone();
                        async move {
                            let n = policy.violations().await;
                            if n > 0 {
                                return Ok(Decision::deny(
                                    1, format!("EPOLICY: {n} prior violation(s)."),
                                ));
                            }
                            Ok(Decision::Resume)
                        }
                    }
                }))

                // Tier-1 sanity: a commit must have dirtied at least one byte
                .after("git commit *", handler(|c: HandlerCtx| async move {
                    // dirty_bytes is always populated; zero is a real measurement
                    if c.receipt().await?.dirty_bytes == 0 {
                        return Err(PluginError::Halt("zero-byte delta commit".into()));
                    }
                    Ok(PostExecAction::Proceed)
                }))
        }
    });

    // ── LIFECYCLE HOOKS (§11.4, §12) ────────────────────────────────────
    let builder = builder
        .on_vm_init(|driver| async move {
            driver.set_watchdog(Duration::from_secs(5 * 60), kelir::WatchdogAction::Reboot).await
        })
        .on_vm_ready(|session: &Session| async move {
            session.exec_in_guest("npm", &["ci", "--prefer-offline"]).await
        })
        .on_vm_shutdown(|session: &Session| async move {
            session.ledger.flush().await
        })
        .on_trap_deny(|event, session: &Session| async move {
            if session.ledger.count_denials(event.ppid).await? > 5 {
                session.vm.suspend("./.kelir/quarantine.snap").await?;
                session.break_glass().await?;
            }
            Ok(())
        });

    Ok(builder)
}

// ============================================================================
// 4. ORCHESTRATION — the seven phases
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let oracle: Arc<dyn Oracle> = Arc::new(UserOracle::new());
    let stripe_key = std::env::var("REAL_HOST_STRIPE_KEY")?;

    let builder = build_runtime(
        Arc::clone(&oracle),
        stripe_key,
        "/etc/ssl/client.crt".into(),
        "/etc/ssl/client.key".into(),
    ).await?;

    // ── Boot: the sole point where the write-only chain becomes a session ─
    let session = builder.boot().await?;
    eprintln!("[kelir] driver={} boot={}ms", session.driver_name(), session.boot_ms());

    // Port forward the dev server for human smoke testing (§11.7)
    session.forward_port(8080, 3000).await?;

    let base = session.vm.checkpoint("clean_base").await?;
    let mut winning_delta: Option<CausalDelta> = None;

    let phase: Result<(), Box<dyn std::error::Error + Send + Sync>> = async {
        // ── Phase 1: hermetic test (chaos on, egress forbidden) ──────────
        let test = session.spawn("npm")
            .args(["test", "--", "--runInBand"])
            .cwd("/workspace/checkout-service")
            // params cross into the guest as the KELIR_PARAMS=<json> environment variable
            .params::<SpawnAction>(SpawnAction::FixStripeWebhook {
                repo: "checkout-service".into(),
                issue_id: "ISSUE-8412".into(),
                env: Env::Staging,
            })
            .await?;
        if test.exit_code != 0 {
            session.vm.rollback(&base).await?;
            session.break_glass().await?;   // §11.6
            return Ok(());
        }

        // ── Phase 2: 4-way CoW fork fan-out (§11.1, §11.3) ──────────────
        let mut joins = tokio::task::JoinSet::new();
        for id in ['A', 'B', 'C', 'D'] {
            let parent = session.clone();
            joins.spawn(async move {
                let child = parent.vm.fork().await?;                 // <15ms
                child.vm
                    .clock
                    .skew(Duration::from_secs(24 * 3600 * (id as u64 - 64)))
                    .await?;
                let run = child.spawn("npm")
                    .args(["run", &format!("verify-hypothesis-{id}")])
                    .cwd("/workspace/checkout-service")
                    .await?;
                let delta = child.get_causal_delta_from_parent().await?;
                child.terminate().await?;
                Ok::<_, Box<dyn std::error::Error + Send + Sync>>((id, run, delta))
            });
        }

        let mut winner: Option<(char, CausalDelta)> = None;
        while let Some(joined) = joins.join_next().await {
            let (id, run, delta) = joined??;
            if run.exit_code == 0 && winner.is_none() {
                winner = Some((id, delta));
            }
        }
        let Some((id, delta)) = winner else {
            session.vm.rollback(&base).await?;
            return Ok(());
        };
        winning_delta = Some(delta);

        // ── Phase 3: suspend to 0 MB RSS, await approval, resume (§11.2) ─
        session.suspend("/tmp/eval.snap").await?;
        eprintln!("[kelir] suspended: host RSS = 0 MB; awaiting approval");
        oracle.await_human_approval().await?;
        session.resume("/tmp/eval.snap").await?;

        // ── Phase 4: overlay reconcile to host, then git commit (§8.1) ─
        let stats = session.fs.reconcile(FsReconcileRequest {
            cwd: "/workspace/checkout-service".into(),
            atomic_staging: true,
            fsync: true,
        }).await?;
        eprintln!(
            "[kelir] overlay reconciled files={} deletes={}",
            stats.files_changed, stats.whiteouts_resolved,
        );

        // Git runs as an ordinary guest execution — the trap chain gates it
        // like any other binary.
        session.spawn("git")
            .args([
                "commit",
                "-am",
                &format!("fix(stripe): dedupe webhook body reads ({id})"),
            ])
            .cwd("/workspace/checkout-service")
            .await?;

        // ── Phase 5: signed Merkle receipt (§12.3) ─────────────────────
        let receipt: FinalReceipt = session
            .ledger
            .finalize(FinalizeRequest {
                include_delta: winning_delta.clone(),
                include_merkle_root: true,
            })
            .await?;
        eprintln!(
            "[kelir] final receipt merkle={} jws={}",
            receipt.merkle_root, &receipt.signed_jws[..64.min(receipt.signed_jws.len())],
        );

        Ok(())
    }.await;

    // ── Phase 6-7: suspend to reclaim RAM, then terminate (§11.4) ───────
    if let Err(e) = session.suspend("/tmp/eval.snap").await {
        eprintln!("[kelir] final suspend failed: {e}");
    }
    session.terminate().await?;
    phase
}

// ============================================================================
// 5. ORACLE (user-supplied post-condition verification engine)
// ============================================================================

struct UserOracle;
impl UserOracle { fn new() -> Self { Self } }

#[async_trait]
impl Oracle for UserOracle {
    async fn analyze_intent(&self, _trap: &TrapEvent) -> Result<IntentAnalysis, Box<dyn std::error::Error + Send + Sync>> {
        Ok(IntentAnalysis {
            is_hypothesis_valid: true,
            falsification_reason: None,
            allow_high_entropy: false,
        })
    }
    async fn verify_physical_receipt(&self, _r: &ExecutionReceipt) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(true)
    }
    async fn ingest_harness_event(
        &self, _p: &Path, _e: &serde_json::Value, _b: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> { Ok(()) }
    async fn await_human_approval(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(())
    }
}
