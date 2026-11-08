#![allow(clippy::unwrap_used)]
#![allow(dead_code)]

// see: cans/02-plugin-model.md#Unified-verdict

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use async_trait::async_trait;
use ciborium::value::Value as CborValue;
use kelir_interface::driver::SuspendReason;
use kelir_interface::plugin::HandlerCtx;

use kelir::{
    HCons, HNil, KelirBuilder, NoParams, PidLineage, SnapshotDescriptor, Unsealed, VmConfig,
    driver::{DriverPolicy, WxPolicy},
    handler,
    plugin::{
        AnyPlugin, KelirPlugin, LifecycleCtx, LifecycleParticipant, PluginError,
        SystemTelemetryParticipant, TrapInterposer, Verdict, VetoEvent, WatchdogEvent,
        WatchdogVerdict,
    },
    protocol::{PatchPayload, SuspendUntil, TrapEvent},
};

#[derive(Default)]

pub struct PolicyState {
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
    pub fn mitigations(&self) -> usize {
        *self
            .state
            .mitigations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }
    pub fn suppressions(&self) -> usize {
        *self
            .state
            .suppressions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }
    pub fn quarantined(&self) -> Vec<u32> {
        self.state
            .quarantined_pids
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub fn set_planned_suspend(&self, v: bool) {
        self.state.planned_suspend.store(v, Ordering::SeqCst);
    }
}

#[derive(Clone)]
pub struct PolicyMatrixPlugin {
    state: Arc<PolicyState>,
}

impl PolicyMatrixPlugin {
    pub fn new() -> Self {
        Self {
            state: Arc::new(PolicyState::default()),
        }
    }
    pub fn handle(&self) -> PolicyHandle {
        PolicyHandle {
            state: Arc::clone(&self.state),
        }
    }
}

impl Default for PolicyMatrixPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for PolicyMatrixPlugin {
    type Context = ();
    fn context(&self) -> Self::Context {}
    fn name(&self) -> &'static str {
        "policy-matrix"
    }

    fn fork_state(&self, _child_session_id: &str) -> Box<dyn AnyPlugin> {
        let state = PolicyState {
            ready: AtomicBool::new(self.state.ready.load(Ordering::SeqCst)),
            planned_suspend: AtomicBool::new(false),
            mitigations: Mutex::new(
                *self
                    .state
                    .mitigations
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()),
            ),
            suppressions: Mutex::new(
                *self
                    .state
                    .suppressions
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()),
            ),
            quarantined_pids: Mutex::new(
                self.state
                    .quarantined_pids
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone(),
            ),
        };
        Box::new(Self {
            state: Arc::new(state),
        })
    }
}

#[async_trait]
impl LifecycleParticipant for PolicyMatrixPlugin {
    async fn on_vm_init(&self, ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        let ready = Arc::clone(&self.state);
        let _ = ctx.register_veto(
            VetoEvent::Suspend,
            Box::new(move || ready.ready.load(Ordering::SeqCst)),
        );
        let ready = Arc::clone(&self.state);
        let _ = ctx.register_veto(
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
    async fn on_trap(&self, event: &TrapEvent, ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        let binary = event
            .binary_path
            .rsplit('/')
            .next()
            .unwrap_or(&event.binary_path);
        match binary {
            "policy-allow" => Ok(Verdict::Pass),

            "policy-deny" => Ok(Verdict::Drop {
                errno: Some("EPOLICY".into()),
                reason: "EPOLICY: policy-matrix denies this binary.".into(),
            }),

            "policy-rewrite" => {
                let mutation = serde_json::json!({
                    "new_binary": "/bin/policy-allow",
                    "new_argv": ["policy-allow", "--rewritten"],
                    "set_env": { "KELIR_POLICY_REWRITE": "1" },
                    "unset_env": [],
                });
                Ok(Verdict::Patch(PatchPayload::ExecveMutation(
                    serde_json::to_vec(&mutation).map_err(|e| PluginError::Halt(e.to_string()))?,
                )))
            }

            "policy-abort" => Ok(Verdict::Abort {
                reason: "EPOLICY: abort row".into(),
            }),

            "policy-yield" => {
                // TODO (L3): checkpoint via the session checkpoint plane; interface HandlerCtx carries no vm handle.
                let _ = ctx;
                Ok(Verdict::Suspend {
                    until: SuspendUntil::Approval,
                })
            }

            "policy-approval" => Ok(Verdict::Suspend {
                until: SuspendUntil::Deadline(500),
            }),

            "policy-custom" => Ok(Verdict::Patch(PatchPayload::Custom {
                name: "quarantine".into(),
                cbor: CborValue::Map(vec![
                    (
                        CborValue::Text("pid".into()),
                        CborValue::Integer(event.pid.into()),
                    ),
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
    async fn on_watchdog(&self, event: &WatchdogEvent) -> Result<WatchdogVerdict, PluginError> {
        if event.last_active_pid.is_some() {
            *self
                .state
                .mitigations
                .lock()
                .unwrap_or_else(|e| e.into_inner()) += 1;
            return Ok(WatchdogVerdict::Mitigate);
        }
        Ok(WatchdogVerdict::ExecuteDefault)
    }
}

#[derive(Clone)]
pub struct WatchdogSilencerPlugin {
    state: Arc<PolicyState>,
}

impl WatchdogSilencerPlugin {
    pub fn new(state: Arc<PolicyState>) -> Self {
        Self { state }
    }
}

impl KelirPlugin for WatchdogSilencerPlugin {
    type Context = ();
    fn context(&self) -> Self::Context {}
    fn name(&self) -> &'static str {
        "watchdog-silencer"
    }
}

#[async_trait]
impl SystemTelemetryParticipant for WatchdogSilencerPlugin {
    async fn on_watchdog(&self, _event: &WatchdogEvent) -> Result<WatchdogVerdict, PluginError> {
        if self.state.planned_suspend.load(Ordering::SeqCst) {
            *self
                .state
                .suppressions
                .lock()
                .unwrap_or_else(|e| e.into_inner()) += 1;
            return Ok(WatchdogVerdict::Suppress);
        }
        Ok(WatchdogVerdict::ExecuteDefault)
    }
}

type PolicyPlugins = HCons<(), HCons<(), HNil>>;

type PolicyBuilder = KelirBuilder<Unsealed, PolicyPlugins, NoParams>;

fn build_runtime() -> Result<PolicyBuilder, Box<dyn std::error::Error + Send + Sync>> {
    let policy = PolicyMatrixPlugin::new();
    let shared = Arc::clone(&policy.state);

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
                                    .unwrap_or_else(|e| e.into_inner())
                                    .push(i128::from(pid) as u32);
                            }
                        }
                    }
                }
                Ok(())
            }
        }),
    )
    .register_stream_kind(
        0x0013,
        serde_json::json!({ "type": "object", "properties": {} }),
        handler(|request: Vec<u8>| async move { Ok(request) }),
    );

    Ok(builder)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut session = build_runtime()?.boot().await?;
    session.terminal.resize(120, 40)?;
    eprintln!("[kelir] host pressure={}", session.host.pressure_level());
    let port = session.wire.forward_port(8080, Some(8080))?;
    eprintln!("[kelir] wire forward_port -> {port}");

    for binary in [
        "policy-allow",
        "policy-deny",
        "policy-rewrite",
        "policy-abort",
        "policy-yield",
        "policy-approval",
        "policy-custom",
    ] {
        let slot = session.checkpoint.create_step(
            PidLineage::new(1),
            SnapshotDescriptor {
                id: format!("policy-matrix-{binary}"),
                dirty_bytes: 0,
                created_at_ns: 0,
                parent_digest: None,
                guest_clock: None,
            },
        )?;
        let node = session.checkpoint.commit_step(PidLineage::new(1))?;
        eprintln!("[kelir] {binary} -> slot={slot} node={node}");
    }
    // TODO (L3): spawn each policy binary in the guest and drive the policy matrix over it.
    // TODO (L3): open a vsock channel on kind 0x0013 and echo policy-matrix:ping through it.

    session.suspend(SuspendReason::ResourcePressure)?;
    session.resume()?;

    session.stop()?;
    Ok(())
}
