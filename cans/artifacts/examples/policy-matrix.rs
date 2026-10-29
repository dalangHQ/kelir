//! examples/policy-matrix.rs
//!
//! Canonical depth example: the full TrapAction lattice, lifecycle vetoes,
//! watchdog verdicts, a custom action, and binary vsock RPC — the surface
//! the swe-agent family deliberately does not touch.
//!
//! SAME CHAIN as policy-matrix.ts and policy-matrix.py. Registration order
//! is the dispatch order; each binary hits exactly one row:
//!
//!   1. policy-allow    -> Allow            (resume untouched)
//!   2. policy-deny     -> Deny            (exit 1, EPOLICY)
//!   3. policy-rewrite  -> Rewrite         (new binary + argv, env ONLY via
//!                                            Rewrite target_mutation
//!                                            (ExecveMutation) — never
//!                                            env.set() + resume, which is
//!                                            EINVALIDDECISION per 09)
//!   4. policy-abort    -> Abort           (pre-exec, exit 1)
//!   5. policy-yield    -> checkpoint "policy-yield", then Yield to host
//!   6. policy-approval -> Hold (500 ms)
//!   7. policy-custom   -> Custom("quarantine", CBOR payload) dispatched to
//!                          builder.register_custom_action("quarantine", …)
//!   8. Vetoes (on_vm_init): Suspend and ClockSkew, each allowed only once
//!      the ready flag is set in on_vm_ready; before that both abort.
//!   9. Watchdog: PolicyMatrixPlugin returns Mitigate when a live PID was
//!      active at fire time; WatchdogSilencerPlugin returns Suppress while
//!      a planned suspend is in progress, else ExecuteDefault.
//!  10. Vsock: builder.register_vsock_service(2, …) + session.vsock.channel(2)
//!      binary RPC round-trip in main().
//!
//! GAPS (spec-documented form used where it exists; the rest is flagged,
//! not silently invented):
//!   - Yield id: String wins (parent decision). TrapAction::Yield
//!     carries checkpoint_id: String, matching SnapshotDescriptor.id
//!     (vmm-driver-trait.rs) and the "policy-yield" label TS/Python
//!     already pass; the yield row below passes "policy-yield".
//!   - builder.register_custom_action / builder.register_vsock_service are
//!     specified in 09/03 but their handler signatures and the schema
//!     argument's type appear in no example; the closures and serde_json
//!     schema values below are the closest documented form, not a claim
//!     about the facade's exact signature.
//!   - session.vsock.channel(2) is specified in 03; the channel's read/write
//!     shape is not shown anywhere. This file treats it as an async byte
//!     stream (the vsock_connect -> UnixStream precedent in
//!     vmm-driver-trait.rs), which is the closest documented form.
//!   - on_trap calls ctx.vm.checkpoint through &HandlerCtx; the swe-agent
//!     example calls checkpoint through an owned HandlerCtx, so whether
//!     the handle method takes &self is unconfirmed by any example.
//!
//! Cargo.toml:
//!   [dependencies]
//!   kelir = { version = "3.0", features = ["ledger"] }
//!   tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "io-util", "time"] }
//!   async-trait = "0.1"
//!   serde = { version = "1", features = ["derive"] }
//!   serde_json = "1"
//!   ciborium = "0.2"

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use async_trait::async_trait;
use ciborium::value::Value as CborValue;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use kelir::{
    handler, DriverChoice, HandlerCtx, KelirBuilder, Session, VmConfig,
    plugin::{
        AnyPlugin, KelirPlugin, LifecycleCtx, LifecycleParticipant,
        PluginError, SystemTelemetryParticipant, TrapInterposer,
        VetoEvent, WatchdogEvent, WatchdogVerdict,
    },
    protocol::{TrapAction, TrapEvent},
};

// ============================================================================
// 1. SHARED STATE — veto flags, watchdog counters, quarantine log
// ============================================================================

#[derive(Default)]
struct PolicyState {
    ready: AtomicBool,
    planned_suspend: AtomicBool,
    mitigations: Mutex<usize>,
    suppressions: Mutex<usize>,
    quarantined_pids: Mutex<Vec<u32>>,
}

#[derive(Clone)]
pub struct PolicyHandle {
    state: Arc<PolicyState>,
}

impl PolicyHandle {
    pub fn mitigations(&self) -> usize { *self.state.mitigations.lock().unwrap() }
    pub fn suppressions(&self) -> usize { *self.state.suppressions.lock().unwrap() }
    pub fn quarantined(&self) -> Vec<u32> { self.state.quarantined_pids.lock().unwrap().clone() }
    pub fn set_planned_suspend(&self, v: bool) { self.state.planned_suspend.store(v, Ordering::SeqCst); }
}

// ============================================================================
// 2. PLUGIN — the lattice lives in on_trap; vetoes in on_vm_init
// ============================================================================

/// Plugin: policy-matrix; rings: virtio-vsock, virtio-fs
pub struct PolicyMatrixPlugin {
    state: Arc<PolicyState>,
}

impl PolicyMatrixPlugin {
    pub fn new() -> Self {
        Self { state: Arc::new(PolicyState::default()) }
    }
    pub fn handle(&self) -> PolicyHandle {
        PolicyHandle { state: Arc::clone(&self.state) }
    }
}

impl Default for PolicyMatrixPlugin {
    fn default() -> Self { Self::new() }
}

impl KelirPlugin for PolicyMatrixPlugin {
    fn name(&self) -> &'static str { "policy-matrix" }

    fn fork_state(&self, _child_session_id: &str) -> Box<dyn AnyPlugin> {
        // Snapshot copy of the counters, following the swe-agent precedent
        // of cloning state at fork; veto flags re-arm per child.
        let state = PolicyState {
            ready: AtomicBool::new(self.state.ready.load(Ordering::SeqCst)),
            planned_suspend: AtomicBool::new(false),
            mitigations: Mutex::new(*self.state.mitigations.lock().unwrap()),
            suppressions: Mutex::new(*self.state.suppressions.lock().unwrap()),
            quarantined_pids: Mutex::new(self.state.quarantined_pids.lock().unwrap().clone()),
        };
        Box::new(Self { state: Arc::new(state) })
    }
}

#[async_trait]
impl LifecycleParticipant for PolicyMatrixPlugin {
    async fn on_vm_init(&self, ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        // Spec form (02: LifecycleCtx exposes register_veto). Both vetoes
        // allow only once on_vm_ready has set the ready flag; before that
        // any Suspend or ClockSkew aborts with ESessionVeto (07).
        let ready = Arc::clone(&self.state);
        ctx.register_veto(
            VetoEvent::Suspend,
            Box::new(move || ready.ready.load(Ordering::SeqCst)),
        );
        let ready = Arc::clone(&self.state);
        ctx.register_veto(
            VetoEvent::ClockSkew,
            Box::new(move || ready.ready.load(Ordering::SeqCst)),
        );
        Ok(())
    }

    async fn on_vm_ready(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        self.state.ready.store(true, Ordering::SeqCst);
        Ok(())
    }
}

#[async_trait]
impl TrapInterposer for PolicyMatrixPlugin {
    async fn on_trap(
        &self, event: &TrapEvent, ctx: &HandlerCtx,
    ) -> Result<Option<TrapAction>, PluginError> {
        let binary = event.binary_path.rsplit('/').next().unwrap_or(&event.binary_path);
        match binary {
            // Row 1: Allow — resume untouched.
            "policy-allow" => Ok(Some(TrapAction::Allow)),
            // Row 2: Deny — exit code + diagnostic payload injected into the TTY.
            "policy-deny" => Ok(Some(TrapAction::Deny {
                exit_code: 1,
                diagnostic_payload: b"EPOLICY: policy-matrix denies this binary.".to_vec(),
            })),
            // Row 3: Rewrite — env mutation ONLY through the ExecveMutation
            // target_mutation payload (plugins/tool-guard.md).
            "policy-rewrite" => {
                let mutation = serde_json::json!({
                    "new_binary": "/bin/policy-allow",
                    "new_argv": ["policy-allow", "--rewritten"],
                    "set_env": { "KELIR_POLICY_REWRITE": "1" },
                    "unset_env": [],
                });
                Ok(Some(TrapAction::Rewrite {
                    target_mutation: serde_json::to_vec(&mutation).unwrap(),
                }))
            }
            // Row 4: Abort — pre-exec, hypervisor-level termination.
            "policy-abort" => Ok(Some(TrapAction::Abort {
                exit_code: 1,
                reason: "EPOLICY: abort row".into(),
            })),
            // Row 5: Yield — checkpoint first, then freeze to the host.
            // checkpoint_id is the String label of that checkpoint.
            "policy-yield" => {
                let _snap = ctx.vm.checkpoint("policy-yield").await?;
                Ok(Some(TrapAction::Yield { checkpoint_id: "policy-yield".into() }))
            }
            // Row 6: Hold — host holds 500 ms pending approval.
            "policy-approval" => Ok(Some(TrapAction::Hold {
                duration_ms: 500,
            })),
            // Row 7: Custom — dispatched by name to the registered handler.
            // Unknown names fail with EUnknownAction (09); "quarantine"
            // is registered on the builder below.
            "policy-custom" => Ok(Some(TrapAction::Custom(CborValue::Map(vec![
                (CborValue::Text("pid".into()), CborValue::Integer(event.pid.into())),
                (
                    CborValue::Text("reason".into()),
                    CborValue::Text("policy-matrix quarantine row".into()),
                ),
            ])))),
            _ => Ok(None),
        }
    }
}

#[async_trait]
impl SystemTelemetryParticipant for PolicyMatrixPlugin {
    async fn on_watchdog(
        &self, event: &WatchdogEvent,
    ) -> Result<WatchdogVerdict, PluginError> {
        // A live PID was active when the watchdog fired: do not run the
        // default Reboot/Terminate action; delegate recovery to the host
        // policy handler (07: Mitigate delegates recovery to host policy).
        if event.last_active_pid.is_some() {
            *self.state.mitigations.lock().unwrap() += 1;
            return Ok(WatchdogVerdict::Mitigate);
        }
        Ok(WatchdogVerdict::ExecuteDefault)
    }
}

// ============================================================================
// 3. SECOND WATCHDOG PLUGIN — Suppress is a real second code path
// ============================================================================

/// Returns Suppress while main() holds a planned suspend open, so a
/// watchdog firing inside the suspend window does not reboot a VM that
/// is intentionally parked. Otherwise it defers to the default action.
pub struct WatchdogSilencerPlugin {
    state: Arc<PolicyState>,
}

impl WatchdogSilencerPlugin {
    pub fn new(state: Arc<PolicyState>) -> Self { Self { state } }
}

impl KelirPlugin for WatchdogSilencerPlugin {
    fn name(&self) -> &'static str { "watchdog-silencer" }

    fn fork_state(&self, _child_session_id: &str) -> Box<dyn AnyPlugin> {
        Box::new(Self { state: Arc::clone(&self.state) })
    }
}

#[async_trait]
impl SystemTelemetryParticipant for WatchdogSilencerPlugin {
    async fn on_watchdog(
        &self, _event: &WatchdogEvent,
    ) -> Result<WatchdogVerdict, PluginError> {
        if self.state.planned_suspend.load(Ordering::SeqCst) {
            *self.state.suppressions.lock().unwrap() += 1;
            return Ok(WatchdogVerdict::Suppress);
        }
        Ok(WatchdogVerdict::ExecuteDefault)
    }
}

// ============================================================================
// 4. BUILDER — custom action + vsock service, then boot
// ============================================================================

fn build_runtime() -> Result<KelirBuilder, Box<dyn std::error::Error + Send + Sync>> {
    let policy = PolicyMatrixPlugin::new();
    let shared = Arc::clone(&policy.state);

    // Custom action "quarantine": the trap row above dispatches here by
    // name. The handler records the pid; the quarantine snapshot itself
    // is taken by main() (a handler receives no &Session per 02's hook
    // context restriction). Handler signature + schema type: see header.
    let quarantine_state = Arc::clone(&shared);
    let builder = KelirBuilder::new(VmConfig {
        vcpus: 2,
        memory_mb: 2048,
        kernel: format!("./fixtures/vmlinux-6.6-{}-kelir", std::env::consts::ARCH).into(),
        rootfs_erofs: "./fixtures/rootfs-policy.erofs".into(),
        tsc_locked: true,
        enable_watchdog: true,
        enable_gpu_passthrough: false,
        driver: DriverChoice::Auto,
        entropy_seed: None,
    })
    .plugin(policy)
    .plugin(WatchdogSilencerPlugin::new(shared))
    .register_custom_action(
        "quarantine",
        serde_json::json!({
            "type": "object",
            "properties": {
                "pid": { "type": "integer" },
                "reason": { "type": "string" },
            },
            "required": ["pid", "reason"],
        }),
        handler(move |payload: CborValue| {
            let quarantine_state = Arc::clone(&quarantine_state);
            async move {
                if let CborValue::Map(entries) = payload {
                    for (k, v) in entries {
                        if k == CborValue::Text("pid".into()) {
                            if let CborValue::Integer(pid) = v {
                                quarantine_state
                                    .quarantined_pids
                                    .lock()
                                    .unwrap()
                                    .push(pid as u32);
                            }
                        }
                    }
                }
                Ok(())
            }
        }),
    )
    // Vsock service on vsock channel kind 2: the RPC peer of session.vsock.channel(2)
    // in main(). Echoes the request frame back to the caller.
    .register_vsock_service(
        2,
        serde_json::json!({ "type": "object", "properties": {} }),
        handler(|request: Vec<u8>| async move { Ok(request) }),
    );

    Ok(builder)
}

// ============================================================================
// 5. MAIN — drive every row in chain order
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let session: Session = build_runtime()?.boot().await?;
    eprintln!("[kelir] driver={} boot={}ms", session.driver_name(), session.boot_ms());

    // Vsock binary RPC (03: session.vsock.channel(2), binary framing).
    let mut channel = session.vsock.channel(2).await?;
    channel.write_all(b"policy-matrix:ping").await?;
    let mut reply = vec![0u8; 64];
    let n = channel.read(&mut reply).await?;
    eprintln!("[kelir] vsock channel 2 reply: {}", String::from_utf8_lossy(&reply[..n]));

    // Rows 1-3 return to the guest; rows 4-7 park, abort, or divert.
    // Each spawn below exercises exactly one trap row, in chain order.
    for binary in [
        "policy-allow",
        "policy-deny",
        "policy-rewrite",
        "policy-abort",
        "policy-yield",
        "policy-approval",
        "policy-custom",
    ] {
        let run = session.spawn(binary).args(["--policy-matrix"]).await?;
        eprintln!("[kelir] {binary} -> exit={}", run.exit_code);
    }

    // Planned suspend window: a watchdog firing here hits the silencer's
    // Suppress path instead of the default Reboot/Terminate action.
    session.suspend("/tmp/policy-matrix.snap").await?;
    session.resume("/tmp/policy-matrix.snap").await?;

    session.terminate().await?;
    Ok(())
}
