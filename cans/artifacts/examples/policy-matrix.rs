#![allow(dead_code)]

// see: cans/02-plugin-model.md#Unified-verdict
// see: cans/08-lifecycle.md#Lifecycle-vetoes
// see: cans/11-protocol.md#Control-channel

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use kelir::async_trait;
use kelir::CborValue;
use kelir::{Deserialize, Serialize};
use kelir_interface::driver_interface::Stream;
use kelir_interface::driver_interface::SuspendReason;
use kelir_interface::plugin_model::HandlerCtx;
use kelir_interface::vsock_mesh::{VsockPeer, VsockServiceHandler, VsockServiceSpec};

use kelir::{
    driver_interface::{DriverPolicy, WxPolicy},
    plugin_model::{
        AnyPlugin, KelirPlugin, LifecycleCtx, LifecycleParticipant, NoDeps, PluginDeps,
        PluginError, SystemTelemetryParticipant, TrapInterposer, Verdict, Veto, VetoEvent,
        WatchdogEvent, WatchdogVerdict,
    },
    protocol_wire::{Errno, ExecveMutation, PatchPayload, SuspendUntil, TrapEvent},
    HCons, HNil, Host, KelirBuilder, NoParams, SnapshotDescriptor, Unsealed, VmConfig,
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

impl PluginDeps for PolicyMatrixPlugin {
    type Deps = NoDeps;
}

#[async_trait]
impl LifecycleParticipant for PolicyMatrixPlugin {
    async fn on_vm_init(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        let ready = Arc::clone(&self.state);
        ctx.register_veto(
            VetoEvent::Suspend,
            Box::new(move || Veto::allowed(ready.ready.load(Ordering::SeqCst))),
        );
        let ready = Arc::clone(&self.state);
        ctx.register_veto(
            VetoEvent::ClockSkew,
            Box::new(move || Veto::allowed(ready.ready.load(Ordering::SeqCst))),
        );
        Ok(())
    }

    async fn on_vm_ready(&self, _ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
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
                errno: Errno::Epolicy,
                reason: "EPOLICY: policy-matrix denies this binary.".into(),
            }),
            // Row 3: Patch — env mutation ONLY through the ExecveMutation
            // payload (plugins/tool-guard.md); the typed struct serializes to
            // CBOR on the wire (11-protocol.md#Wire-framing).
            "policy-rewrite" => Ok(Verdict::Patch(PatchPayload::ExecveMutation(
                ExecveMutation {
                    new_binary: Some("/bin/policy-allow".into()),
                    new_argv: Some(vec!["policy-allow".into(), "--rewritten".into()]),
                    set_env: vec![("KELIR_POLICY_REWRITE".into(), "1".into())],
                    unset_env: vec![],
                },
            ))),
            // Row 4: Abort — pre-exec, hypervisor-level termination.
            // Abort drops exit_code, keeps reason only.
            "policy-abort" => Ok(Verdict::Abort {
                reason: "EPOLICY: abort row".into(),
            }),

            "policy-yield" => {
                let _snap = ctx.vm.checkpoint("policy-yield").await?;
                Ok(Verdict::Suspend {
                    until: SuspendUntil::Approval,
                })
            }

            "policy-approval" => Ok(Verdict::Suspend {
                until: SuspendUntil::Deadline(500),
            }),

            // Row 7: Signed Operator Lease Escalation (Permits widening via Ed25519 signature)
            "policy-escalate" => {
                if ctx.has_valid_operator_lease() {
                    Ok(Verdict::Pass)
                } else {
                    Ok(Verdict::Drop {
                        errno: Errno::Eperm,
                        reason: "ESECURITYVIOLATION: capability widening requires signed operator lease".into(),
                    })
                }
            }

            // Row 8: In-Guest Syscall/Protocol Block (Hardware attack surface reduction)
            "policy-syscall-block" => Ok(Verdict::Drop {
                errno: Errno::Eafnosupport,
                reason: "EAFNOSUPPORT: socket family AF_RXRPC blocked (CVE mitigation)".into(),
            }),

            "policy-custom" => Ok(Verdict::Patch(PatchPayload::Custom {
                name: "quarantine".into(),
                cbor: CborValue::serialized(&QuarantinePayload {
                    pid: event.pid,
                    reason: "policy-matrix quarantine row".into(),
                })
                .map_err(|err| {
                    PluginError::Halt(format!("quarantine payload encode failed: {err}"))
                })?,
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

impl PluginDeps for WatchdogSilencerPlugin {
    type Deps = NoDeps;
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

type PolicyBuilder = KelirBuilder<Host, Unsealed, (), HNil, PolicyPlugins, NoParams>;

#[derive(Deserialize, Serialize)]
#[serde(crate = "kelir::serde")]
struct QuarantinePayload {
    pid: u32,
    reason: String,
}

fn build_runtime(
) -> Result<(PolicyBuilder, Arc<PolicyState>), Box<dyn std::error::Error + Send + Sync>> {
    struct EchoVsock;

    #[async_trait]
    impl VsockServiceHandler for EchoVsock {
        async fn on_connect(
            &self,
            _peer: &VsockPeer,
            mut stream: Box<dyn Stream>,
            _ctx: &HandlerCtx,
        ) -> Result<(), PluginError> {
            let mut request = Vec::new();
            let mut chunk = [0u8; 1024];
            loop {
                match stream.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(read) => request.extend_from_slice(&chunk[..read]),
                    Err(error) => return Err(PluginError::Io(error)),
                }
            }
            let mut written = 0;
            while written < request.len() {
                match stream.write(&request[written..]) {
                    Ok(0) => {
                        return Err(PluginError::Halt(
                            "vsock service: stream closed while writing response".into(),
                        ));
                    }
                    Ok(count) => written += count,
                    Err(error) => return Err(PluginError::Io(error)),
                }
            }
            stream.flush().map_err(PluginError::Io)
        }
    }

    let policy = PolicyMatrixPlugin::new();
    let shared = Arc::clone(&policy.state);

    let quarantine_state = Arc::clone(&shared);
    let builder = KelirBuilder::new(VmConfig {
        tenant_id: None,
        vcpus: 2,
        memory_mb: 2048,
        kernel_path: format!("./fixtures/vmlinux-6.6-{}-kelir", std::env::consts::ARCH).into(),
        rootfs_erofs_path: "./fixtures/rootfs-policy.erofs".into(),
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
    })
    .plugin_instance(policy)
    .plugin_instance(WatchdogSilencerPlugin::new(Arc::clone(&shared)))
    .actions(|a| {
        a.register(
            "quarantine",
            kelir::serde_json::json!({
                "type": "object",
                "properties": {
                    "pid": { "type": "integer" },
                    "reason": { "type": "string" },
                },
                "required": ["pid", "reason"],
            }),
            move |payload: QuarantinePayload| {
                let quarantine_state = Arc::clone(&quarantine_state);
                async move {
                    quarantine_state
                        .quarantined_pids
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push(payload.pid);
                    Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
                }
            },
        )
        .end()
    })
    .vsock(|v| {
        v.service(
            VsockServiceSpec {
                port: 0x0013,
                service: "echo".to_string(),
                schema: None,
            },
            EchoVsock,
        )
        .end()
    });

    Ok((builder, shared))
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    kelir::block_on(async_main())?
}

async fn async_main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (builder, shared) = build_runtime()?;
    let mut session = builder.seal()?.boot().await?;
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
            session.lineage(1),
            SnapshotDescriptor {
                id: format!("policy-matrix-{binary}"),
                dirty_bytes: 0,
                created_at_ns: 0,
                parent_digest: None,
                guest_clock: None,
            },
        )?;
        let node = session.checkpoint.commit_step(session.lineage(1))?;
        eprintln!("[kelir] {binary} -> slot={slot} node={node}");
    }

    // 1. Multiplexed binary vsock ping-pong
    let mut channel = session.vsock.channel(0x0013).await?;
    channel.write_all(b"policy-matrix:ping").await?;
    let mut reply = vec![0u8; 18];
    channel.read_exact(&mut reply).await?;
    eprintln!(
        "[kelir] vsock channel 0x0013 reply: {}",
        String::from_utf8_lossy(&reply)
    );
    assert_eq!(&reply, b"policy-matrix:ping");

    // 2. Drive the unified Verdict lattice through guest execve traps
    for binary in [
        "policy-allow",
        "policy-deny",
        "policy-rewrite",
        "policy-abort",
        "policy-yield",
        "policy-approval",
        "policy-escalate",
        "policy-syscall-block",
        "policy-custom",
    ] {
        let run = session.spawn(binary, &["--policy-matrix"], None).await?;
        eprintln!("[kelir] {binary} -> exit={}", run.exit_code);
        match binary {
            "policy-allow" => assert_eq!(run.exit_code, 0),
            "policy-deny" => assert_ne!(run.exit_code, 0),
            "policy-rewrite" => assert_eq!(run.exit_code, 0),
            "policy-abort" => assert_ne!(run.exit_code, 0),
            "policy-yield" => assert_eq!(run.exit_code, 0),
            "policy-approval" => assert_eq!(run.exit_code, 0),
            "policy-escalate" => {
                // Prior to lease elevation: must fail closed with EPERM
                assert_ne!(run.exit_code, 0);
                // Inject signed operator lease and verify runtime elevation
                session.elevate_lease("signed_jws_operator_lease_token_payload")?;
                let elevated = session.spawn(binary, &["--policy-matrix"]).await?;
                assert_eq!(elevated.exit_code, 0);
            }
            "policy-syscall-block" => {
                assert_ne!(run.exit_code, 0);
                assert_eq!(run.exit_kind, ExitKind::Signaled(libc::SIGSYS as u32));
            }
            "policy-custom" => assert_eq!(run.exit_code, 0),
            _ => {}
        }

        // Verify hardware-anchored ExecutionReceipt is directly available without LedgerPlugin
        let receipt = run.receipt().expect("hardware receipt missing from execution");
        assert!(receipt.wall_clock_ns > 0);
        assert!(!receipt.binary_blake3.is_empty());
    }

    // Verify Custom action triggered and recorded state
    assert!(
        !shared.quarantined_pids.lock().unwrap().is_empty(),
        "custom quarantine action failed to fire"
    );

    session.suspend(SuspendReason::ResourcePressure).await?;
    session.resume().await?;

    session.stop().await?;
    Ok(())
}
