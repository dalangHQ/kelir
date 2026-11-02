//! examples/policy-matrix.rs
//!
//! Canonical depth example: the full Verdict lattice, lifecycle vetoes,
//! watchdog verdicts, a custom action, and binary vsock RPC — the surface
//! the swe-agent family deliberately does not touch.
//!
//! SAME CHAIN as policy-matrix.ts and policy-matrix.py. Registration order
//! is the dispatch order; each binary hits exactly one row:
//!
//!   1. policy-allow    -> Pass            (proceed untouched)
//!   2. policy-deny     -> Drop           (errno EPOLICY, reason)
//!   3. policy-rewrite  -> Patch          (new binary + argv, env ONLY via
//!                                            the ExecveMutation payload
//!                                            — never env.set() + pass,
//!                                            which is EINVALIDDECISION
//!                                            per 09)
//!   4. policy-abort    -> Abort           (pre-exec, reason only)
//!   5. policy-yield    -> checkpoint "policy-yield" taken first, then
//!                          Suspend{until: SuspendUntil::Approval}
//!   6. policy-approval -> Suspend        (until Deadline(500))
//!   7. policy-custom   -> Patch(PatchPayload::Custom{name, cbor}) dispatched to
//!                          builder.register_custom_action("quarantine", …)
//!   8. Vetoes (on_vm_init): Suspend and ClockSkew, each allowed only once
//!      the ready flag is set in on_vm_ready; before that both abort.
//!   9. Watchdog: PolicyMatrixPlugin returns Mitigate when a live PID was
//!      active at fire time; WatchdogSilencerPlugin returns Suppress while
//!      a planned suspend is in progress, else ExecuteDefault.
//!  10. Vsock: builder.register_vsock_service(0x0013, …) + session.vsock.channel(0x0013)
//!      binary RPC round-trip in main().
//!
//! NOTES:
//!   - Suspend{until: SuspendUntil::Approval}: checkpoint taken first, then suspend
//!     awaiting host approval/external input (see row 5).
//!   - register_custom_action / register_vsock_service take a JSON schema
//!     and an async handler; shapes below mirror 09/03.
//!   - session.vsock.channel(kind) is an async byte stream, per
//!     VmmDriver::vsock_connect (UnixStream).
//!   - ctx.vm.checkpoint takes &self.
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
    handler, driver::{DriverPolicy, WxPolicy}, HandlerCtx, KelirBuilder, Session, VmConfig,
    plugin::{
        AnyPlugin, KelirPlugin, LifecycleCtx, LifecycleParticipant,
        PluginError, SystemTelemetryParticipant, TrapInterposer,
        VetoEvent, Verdict, WatchdogEvent, WatchdogVerdict,
    },
    protocol::{PatchPayload, SuspendUntil, TrapEvent},
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
        // any Suspend or ClockSkew aborts with ESessionVeto (08).
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
    ) -> Result<Verdict, PluginError> {
        let binary = event.binary_path.rsplit('/').next().unwrap_or(&event.binary_path);
        match binary {
            // Row 1: Pass — proceed untouched.
            "policy-allow" => Ok(Verdict::Pass),
            // Row 2: Drop — EPOLICY diagnostic payload becomes
            // Drop { errno: "EPOLICY", reason }.
            "policy-deny" => Ok(Verdict::Drop {
                errno: Some("EPOLICY"),
                reason: "EPOLICY: policy-matrix denies this binary.".into(),
            }),
            // Row 3: Patch — env mutation ONLY through the ExecveMutation
            // payload (plugins/tool-guard.md).
            "policy-rewrite" => {
                let mutation = serde_json::json!({
                    "new_binary": "/bin/policy-allow",
                    "new_argv": ["policy-allow", "--rewritten"],
                    "set_env": { "KELIR_POLICY_REWRITE": "1" },
                    "unset_env": [],
                });
                Ok(Verdict::Patch(PatchPayload::ExecveMutation(serde_json::to_vec(&mutation).unwrap())))
            }
            // Row 4: Abort — pre-exec, hypervisor-level termination.
            // Abort drops exit_code, keeps reason only.
            "policy-abort" => Ok(Verdict::Abort {
                reason: "EPOLICY: abort row".into(),
            }),
            // Row 5: Suspend — checkpoint taken first, then suspend
            // awaiting host approval/external input.
            "policy-yield" => {
                let _snap = ctx.vm.checkpoint("policy-yield").await?;
                Ok(Verdict::Suspend {
                    until: SuspendUntil::Approval,
                })
            }
            // Row 6: Suspend — host holds 500 ms pending approval.
            "policy-approval" => Ok(Verdict::Suspend {
                until: SuspendUntil::Deadline(500),
            }),
            // Row 7: Patch(PatchPayload::Custom{name, cbor}) — dispatched by name to
            // the registered handler. Unknown names fail with
            // EUnknownAction (11); "quarantine" is registered on the
            // builder below.
            "policy-custom" => Ok(Verdict::Patch(PatchPayload::Custom {
                name: "quarantine",
                cbor: CborValue::Map(vec![
                    (CborValue::Text("pid".into()), CborValue::Integer(event.pid.into())),
                    (
                        CborValue::Text("reason".into()),
                        CborValue::Text("policy-matrix quarantine row".into()),
                    ),
                ]),
            })),
            _ => Ok(Verdict::Pass),
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
        kernel_path: format!("./fixtures/vmlinux-6.6-{}-kelir", std::env::consts::ARCH).into(),
        rootfs_erofs_path: "./fixtures/rootfs-policy.erofs".into(),
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
    // Vsock service on plugin kind 0x0013: the RPC peer of session.vsock.channel(0x0013)
    // in main(). Echoes the request frame back to the caller.
    .register_vsock_service(
        0x0013,
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

    // Vsock binary RPC (03: session.vsock.channel(0x0013), binary framing).
    let mut channel = session.vsock.channel(0x0013).await?;
    channel.write_all(b"policy-matrix:ping").await?;
    let mut reply = vec![0u8; 64];
    let n = channel.read(&mut reply).await?;
    eprintln!("[kelir] vsock channel 0x0013 reply: {}", String::from_utf8_lossy(&reply[..n]));

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
