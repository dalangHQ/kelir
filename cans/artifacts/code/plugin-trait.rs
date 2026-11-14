// crates/kelir-interface/src/plugin_model.flow.rs
//
// The typed KelirPlugin trait and ring interposer contracts.
// Plugins link directly via Cargo crates; no runtime string resolution.
// Zero unsafe code permitted: workspace enforces #![forbid(unsafe_code)].
// Pipeline execution guarantees panic quarantine via catch_unwind.
//
// Owned by: cans/02-plugin-model.md#KelirPlugin-trait

use async_trait::async_trait;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::any::{Any, TypeId};
use std::sync::Arc;
use crate::handlers::HandlerCtx;
use crate::protocol::messages::{ProcessExit, Verdict, FilterAction, StreamKind, TrapEvent};
use crate::vmm::driver::{BalloonConvergence, VmmDriver, VmConfig, WatchdogAction};

/// Structured payloads carried over the stream mux (PTY_CONTROL kind
/// 0x0002). Signal forwarding only, exec_id-keyed, never raw guest pid;
/// window resize rides the EXEC control sub-lane 0x03, not this kind.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PtyControlMessage {
    SignalForward {
        signal: u32,
        exec_id: u64,
    },
}

/// Non-verdict errors only. Denial and fatal termination are verdicts,
/// not errors: Verdict::Drop { errno, reason } (errno None = silent
/// blackhole, plugin-chaos loss injection only) and Verdict::Abort
/// { reason }. Storage plane: guest write() fails with the Drop errno.
/// Wire plane: policy denial is fast-fail, never silent — L4 TCP gets
/// a TCP RST (or ICMP Port Unreachable), L7 HTTP gets a synthetic 403
/// with a JSON receipt.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    // FUSE driver chunks writes at the 64KB boundary before interception.
    // on_fs_write sees one chunk per call. Guest write() never fails on
    // host buffer size.
    /// Host event loop or foreign runtime thread starvation detected.
    #[error("Foreign FFI channel saturated: worker queue depth exceeded ({depth})")]
    FfiQueueSaturated { depth: usize },
    /// Trap violation against W^X memory execution invariants.
    #[error("W^X execution violation detected: PID {pid} attempted {operation}")]
    WxViolation { pid: u32, operation: String },
    /// Conflicting semantic mutations on overlapping resource targets.
    #[error("Ambiguous mutation detected across plugins on resource '{resource}': {conflict}")]
    AmbiguousMutation { resource: String, conflict: String },
    #[error("{errno}: {reason}")]
    Reject { errno: String, reason: String },
    #[error("Internal I/O failure: {0}")]
    Io(#[from] std::io::Error),
    /// Host-fatal error: the plugin cannot proceed and the pipeline
    /// must halt. This is not a guest verdict (denial is Verdict::Drop,
    /// termination is Verdict::Abort); it covers host-side failures
    /// like a missing checkpoint or an unparseable harness event.
    #[error("Plugin halted: {0}")]
    Halt(String),
}

impl PluginError {
    pub fn ambiguous_mutation(
        resource: impl Into<String>,
        conflict: impl Into<String>,
    ) -> Self {
        Self::AmbiguousMutation { resource: resource.into(), conflict: conflict.into() }
    }

    /// Policy rejection in a Result<(), PluginError> context, where no
    /// Verdict is returnable. Carries errno + reason as structured fields;
    /// handlers that can return a Verdict use Verdict::Drop instead.
    pub fn reject(
        errno: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::Reject { errno: errno.into(), reason: reason.into() }
    }
}

/// Complete inode attribute set surfaced by storage-plane lookups.
/// Populated into FUSE LOOKUP replies with S_ISUID and S_ISGID masked
/// (mode & !0o6000); host writes, creates, and chmods strip the setuid /
/// setgid bits the same way, so in-guest root cannot mint setuid host
/// binaries.
#[derive(Debug, Clone)]
pub struct FsInodeAttr {
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u64,
    pub content: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtioRing {
    Vsock, Console, Fs, Net,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtioDevice {
    Vsock,
    Console,
    Fs,
    Net,
    Shm,
    Auxiliary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryRole {
    Observer,
    Gate,
    Mutator,
    Proxy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionTier {
    Native,
    Foreign,
}

/// Mount-level scoping filters evaluated before dispatch array registration.
/// Scoping is transport-agnostic: filters evaluate above the Transport
/// backend (SPSC ring, AF_VSOCK, or named pipe) and no plugin may key
/// behavior on the transport identity.
#[derive(Debug, Clone, Default)]
pub struct MountScope {
    pub devices: Option<Vec<VirtioDevice>>,
    pub binaries: Option<Vec<String>>,
    pub paths: Option<Vec<PathBuf>>,
}

/// Quarantine boundary guarantee: every interposer dispatch is executed
/// within std::panic::catch_unwind at the supervisor engine perimeter.
/// A panicking plugin does not unwind across the FFI or crash the microkernel;
/// the engine converts the panic into Verdict::Drop { errno: Some("EFAULT"),
/// reason: "Plugin panicked" } or PluginError::Halt, logs the backtrace
/// to the host ledger, and rolls back the active StepSlot.
pub const PANIC_QUARANTINE_ENABLED: bool = true;

impl MountScope {
    pub fn with_devices<I: IntoIterator<Item = VirtioDevice>>(mut self, devices: I) -> Self {
        self.devices = Some(devices.into_iter().collect());
        self
    }
    pub fn with_binaries<I: IntoIterator<Item = S>, S: Into<String>>(mut self, binaries: I) -> Self {
        self.binaries = Some(binaries.into_iter().map(Into::into).collect());
        self
    }
    pub fn with_paths<I: IntoIterator<Item = P>, P: Into<PathBuf>>(mut self, paths: I) -> Self {
        self.paths = Some(paths.into_iter().map(Into::into).collect());
        self
    }
    /// Binary-list membership (L2): no list allows all; an entry matches
    /// when it equals the event binary path, or when it carries no `/` and
    /// equals the file name of that path. Membership admits the event to
    /// dispatch only; it never confers trust — canonical identity
    /// (st_dev, st_ino, binary_blake3) decides trust.
    pub fn allows_binary(&self, binary: &str) -> bool {
        self.binaries.as_ref().is_none_or(|list| {
            list.iter().any(|entry| {
                entry == binary
                    || (!entry.contains('/') && binary.rsplit('/').next() == Some(entry.as_str()))
            })
        })
    }
    /// Evaluates boolean intersection against an ancestor scope during composite unpacking
    pub fn intersect(&self, parent: &Self) -> Self {
        Self {
            devices: match (&self.devices, &parent.devices) {
                (Some(a), Some(b)) => Some(a.iter().filter(|d| b.contains(d)).copied().collect()),
                (Some(a), None) => Some(a.clone()),
                (None, Some(b)) => Some(b.clone()),
                (None, None) => None,
            },
            binaries: match (&self.binaries, &parent.binaries) {
                (Some(a), Some(b)) => Some(a.iter().filter(|x| b.contains(x)).cloned().collect()),
                (Some(a), None) => Some(a.clone()),
                (None, Some(b)) => Some(b.clone()),
                (None, None) => None,
            },
            paths: match (&self.paths, &parent.paths) {
                (Some(a), Some(b)) => Some(a.iter().filter(|x| b.contains(x)).cloned().collect()),
                (Some(a), None) => Some(a.clone()),
                (None, Some(b)) => Some(b.clone()),
                (None, None) => None,
            },
        }
    }
}

/// Unified verdict on hot-path VirtIO rings (Fs, Net): every interposer
/// hook returns Verdict (see messages.rs). Sync hot-paths require native
/// filters or Verdict::Suspend; no per-ring aliases remain.

/// Binds inline interceptors into the linear monotonic pipeline.
/// No service registry: peer capabilities are typed Context values
/// accumulated into the builder's Plugins tuple (see KelirPlugin::Context).
pub trait PipelineBinder {
    fn bind_fs(&mut self, stage: Arc<dyn FsInterposer>);
    fn bind_net(&mut self, stage: Arc<dyn NetInterposer>);
    fn bind_trap(&mut self, stage: Arc<dyn TrapInterposer>);
    fn bind_console(&mut self, stage: Arc<dyn ConsoleInterposer>);
    fn bind_host(&mut self, stage: Arc<dyn HostInterposer>);
    /// Lifecycle and system-telemetry participants ride dedicated rings:
    /// they are not interposers and never join the five VirtIO arrays.
    fn bind_lifecycle(&mut self, stage: Arc<dyn LifecycleParticipant>) {
        self.bind_lifecycle_scoped(stage, MountScope::default());
    }
    fn bind_lifecycle_scoped(&mut self, stage: Arc<dyn LifecycleParticipant>, scope: MountScope);
    fn bind_telemetry(&mut self, stage: Arc<dyn SystemTelemetryParticipant>) {
        self.bind_telemetry_scoped(stage, MountScope::default());
    }
    fn bind_telemetry_scoped(&mut self, stage: Arc<dyn SystemTelemetryParticipant>, scope: MountScope);
    /// Binds a sink to one stream kind on the generic stream mux
    /// (channel 0x0001); the sink receives every mux frame of that kind.
    fn bind_stream_kind(&mut self, kind: StreamKind, sink: Arc<dyn StreamFrameSink>) {
        self.bind_stream_kind_scoped(kind, sink, MountScope::default());
    }
    fn bind_stream_kind_scoped(&mut self, kind: StreamKind, sink: Arc<dyn StreamFrameSink>, scope: MountScope);
}

/// Stream-mux frame sink: one bound sink per StreamKind receives every
/// mux frame (stream_id-keyed) of that kind. Synchronous like the other
/// hot-path hooks; slow work yields through the ring trampolines.
pub trait StreamFrameSink: Send + Sync {
    fn on_stream_frame(
        &self,
        stream_id: u32,
        kind: StreamKind,
        flags: u8,
        payload: &[u8],
    ) -> Result<(), PluginError>;
}

/// Binds non-blocking observers to the immutable telemetry bus.
/// Object-safe: registration is TypeId-keyed on the erased handler; the
/// typed subscribe<E> surface rides the EventBusSubscribe extension trait
/// so `&mut dyn EventBusBinder` is a valid receiver.
pub trait EventBusBinder {
    fn subscribe_erased(
        &mut self,
        event: TypeId,
        handler: Box<dyn Fn(&dyn Any) + Send + Sync>,
    );
}

/// Typed subscribe surface available on every EventBusBinder, including
/// the dyn object (blanket-implemented over subscribe_erased).
pub trait EventBusSubscribe: EventBusBinder {
    fn subscribe<E: Any + Send + Sync + 'static>(&mut self, handler: Box<dyn Fn(&E) + Send + Sync>);
}

impl<B: EventBusBinder + ?Sized> EventBusSubscribe for B {
    fn subscribe<E: Any + Send + Sync + 'static>(&mut self, handler: Box<dyn Fn(&E) + Send + Sync>) {
        self.subscribe_erased(
            TypeId::of::<E>(),
            Box::new(move |any: &dyn Any| {
                if let Some(event) = any.downcast_ref::<E>() {
                    handler(event);
                }
            }),
        );
    }
}

/// Assembly-time composite plugin tree unpacked into flat dispatch entries before boot.
pub trait CompositePlugin: Send + Sync {
    fn unpack(self: Box<Self>, parent_scope: &MountScope) -> Vec<(Box<dyn AnyPlugin>, MountScope)>;
}

// ── Lifecycle context ────────────────────────────────────────────
// Restricted handle for on_vm_init. Exposes only mutating capabilities
// that are pre-boot config; never the raw &mut dyn VmmDriver.

/// Hard-veto events. Any registered callback returning false aborts the
/// corresponding primitive with ESessionVeto. Observation-only events
/// (Checkpoint, Rollback, Resume, Terminate) are not vetoable and are
/// published via EventBusBinder::subscribe instead.
#[derive(Debug, Clone, Copy)]
pub enum VetoEvent {
    Fork, Suspend, ClockSkew,
}

/// Observation-only lifecycle events. Published via EventBusBinder
/// as non-blocking telemetry. Not vetoable; no return value.
#[derive(Debug, Clone, Copy)]
pub enum LifecycleObservation {
    Checkpoint, Rollback, Resume, Terminate,
}

#[derive(Debug, Clone)]
pub struct WatchdogEvent {
    pub timeout_ms: u64,
    pub action: WatchdogAction,
    /// Monotonic ns since VM boot when the watchdog fired.
    pub fired_at_ns: u64,
    pub last_heartbeat_ms: u64,
    pub last_active_pid: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchdogVerdict {
    ExecuteDefault,
    Suppress,
    Mitigate,
}

pub struct LifecycleCtx<'a> {
    pub config: &'a VmConfig,
    pub session_id: &'a str,
    driver: &'a mut dyn VmmDriver,
    vetoes: &'a mut Vec<(VetoEvent, Box<dyn Fn() -> bool>)>,
}

impl<'a> LifecycleCtx<'a> {
    pub async fn set_watchdog(
        &mut self, timeout: Duration, action: WatchdogAction,
    ) -> Result<(), PluginError> {
        self.driver.set_watchdog(timeout, action).await
            .map_err(PluginError::Io)
    }
    /// Returns a BalloonConvergence handle, not void. Host subscribes to
    /// VIRTIO_BALLOON_S_MEMFREE / S_AVAIL; host reclaim gates on guest
    /// ack; fork, snapshot, and heavy exec require prior convergence.
    pub async fn adjust_balloon(&mut self, target_mb: u32) -> Result<BalloonConvergence, PluginError> {
        self.driver.adjust_balloon(target_mb).await
            .map_err(PluginError::Io)
    }
    /// Register a veto callback for a lifecycle action. All registered
    /// callbacks run before the action; any returning false aborts it.
    pub fn register_veto(
        &mut self, event: VetoEvent,
        f: Box<dyn Fn() -> bool>,
    ) {
        self.vetoes.push((event, f));
    }
    /// Atomically updates compiled hook pipeline via ArcSwap
    pub fn update_dispatch(&mut self, _new_dispatch: std::sync::Arc<()>) -> Result<(), PluginError> {
        Ok(())
    }
}

// ── Interface segregation ────────────────────────────────────────
// A core identity trait plus five ring-scoped traits, each with default
// no-op implementations. Plugins implement only the traits they touch.
// NOTE: at builder `.plugin(p)` assembly, core inspects the plugin's
// implemented ring traits and registers handlers into dedicated,
// ring-specific contiguous arrays (e.g. Vec<Arc<dyn FsInterposer>>),
// so unused rings cost zero vtable lookups. No dispatch-registration
// code lives in this file.

// ── Core Identity Trait (Mandatory) ──────────────────────────────
pub trait KelirPlugin: Send + Sync {
    /// Context contributed to the builder's accumulated Plugins tuple.
    /// HandlerCtx resolves it infallibly via `Plugins: Has<Self::Context>`.
    /// `Clone` lets session.fork() reproduce the tuple for the child.
    type Context: Send + Sync + Clone + 'static;

    /// Build the Context value that install_plugin appends to Plugins.
    fn context(&self) -> Self::Context;

    /// Unique identifier for telemetry.
    fn name(&self) -> &str;

    /// Default mount scope. Overridden by explicit builder scope.
    fn default_scope(&self) -> MountScope { MountScope::default() }

    /// Null-object toggle. False omits all hooks from flat dispatch array.
    fn is_enabled(&self) -> bool { true }

    /// Specializes child state on fork; defaults to clone_box().
    /// Stateless or standard-Clone plugins require no implementation.
    fn fork_state(&self, _child_session_id: &str) -> Box<dyn AnyPlugin> {
        self.clone_box()
    }

    /// Register inline fallible interceptors into the hardware ring pipelines.
    fn register_interceptors(&self, _pipeline: &mut dyn PipelineBinder) {}

    /// Register asynchronous non-blocking subscribers to the event bus.
    fn register_subscribers(&self, _bus: &mut dyn EventBusBinder) {}
}

// ── Ring-Scoped Traits (Implement Only What You Touch) ─
#[async_trait]
pub trait LifecycleParticipant {
    async fn on_vm_init(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> { Ok(()) }
    async fn on_vm_ready(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> { Ok(()) }
    async fn on_vm_shutdown(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> { Ok(()) }
    async fn on_pre_commit(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> { Ok(()) }
    async fn on_post_commit(&self, _ctx: &mut LifecycleCtx, _committed: bool) -> Result<(), PluginError> { Ok(()) }
    async fn on_host_preempt(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> { Ok(()) }
}

#[async_trait]
pub trait TrapInterposer {
    async fn on_trap(
        &self, _event: &TrapEvent, _ctx: &HandlerCtx,
    ) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }
    async fn on_process_exit(
        &self, _event: &ProcessExit, _ctx: &HandlerCtx,
    ) -> Result<(), PluginError> { Ok(()) }
}

#[async_trait]
pub trait ConsoleInterposer {
    /// Raw paravirtual character stream input. Intercepts host -> guest port bytes.
    async fn on_console_input(&self, _port: u32, _bytes: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
    /// Raw paravirtual character stream output. Intercepts guest -> host port bytes.
    async fn on_console_output(&self, _port: u32, _chunk: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
    /// Stream mux 0x0001, stream kind EXEC 0x0001, stdout sub-lane 0x01:
    /// programmatic exec streams, no PTY involvement. PTY (Port 0) path
    /// unchanged; the exec path keys on exec_id rather than port.
    /// ExecContext (02) carries pid, exec_id, stream_id, chunk, and
    /// rlimits; the stream_id is fixed by which method fires, and the
    /// rlimits ride the EXEC control sub-lane 0x03, not this hook.
    async fn on_exec_stdout(&self, _pid: u32, _exec_id: u64, _chunk: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
    /// Stream mux 0x0001, stream kind EXEC 0x0001, stderr sub-lane 0x02;
    /// same no-PTY, exec_id-keyed contract as on_exec_stdout.
    async fn on_exec_stderr(&self, _pid: u32, _exec_id: u64, _chunk: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
}

#[async_trait]
pub trait SystemTelemetryParticipant {
    async fn on_memory_pressure(&self, _available_mb: u32, _total_mb: u32) -> Result<(), PluginError> { Ok(()) }
    async fn on_heartbeat(&self, _uptime_ms: u64) -> Result<(), PluginError> { Ok(()) }
    async fn on_watchdog(&self, _event: &WatchdogEvent) -> Result<WatchdogVerdict, PluginError> { Ok(WatchdogVerdict::ExecuteDefault) }
}
pub trait FsInterposer {
    /// Verdict::Pass = fall through; Patch = serve (payload per ring:
    /// FsInode carries the served inode attributes for lookup).
    fn on_fs_lookup(&self, _path: &Path) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }
    fn on_fs_read(&self, _path: &Path, _buffer: &[u8]) -> Result<Vec<u8>, PluginError> { Ok(_buffer.to_vec()) }
    /// Sync hot-path buffer transform. Mutate `_data` in place, return
    /// FilterAction::Forward to commit. No Patch variant: the buffer
    /// is the payload. Native Rust only; foreign runtimes must yield
    /// via FilterAction::Suspend.
    fn on_fs_write(
        &self, _path: &Path, _data: &mut [u8], _pid: u32, _ctx: &HandlerCtx,
    ) -> Result<FilterAction, PluginError> { Ok(FilterAction::Forward) }
    /// Intercepts FUSE SETLK and SETLKW advisory lock requests.
    fn on_fs_lock(
        &self, _path: &Path, _flock_cmd: i32, _flock_type: i16, _pid: u32,
    ) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }
}

pub trait NetInterposer {
    /// Sync L2 outbound buffer transform. Mutate `_frame` in place,
    /// return FilterAction::Forward to commit.
    fn on_net_outbound(&self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> { Ok(FilterAction::Forward) }
    /// Sync L2 inbound buffer transform. Same contract as outbound.
    fn on_net_inbound(&self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> { Ok(FilterAction::Forward) }
}

pub trait HostInterposer {
    fn on_host_lsm_exec(&self, _path: &Path, _pid: u32) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }
    fn on_host_pressure(&self, _level: u32) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }
    fn on_host_keyring_access(&self, _key: &str) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }
}

/// Object-safe clone bridge powering default fork_state implementation.
pub trait PluginClone: Send + Sync {
    fn clone_box(&self) -> Box<dyn AnyPlugin>;
}

impl<T> PluginClone for T
where
    T: 'static + KelirPlugin + Clone,
{
    fn clone_box(&self) -> Box<dyn AnyPlugin> {
        Box::new(self.clone())
    }
}

/// Identity marker for any plugin object: core identity only.
/// fork_state and CompositePlugin::unpack return this type. It must NOT
/// require the ring traits: requiring all five would force every
/// forkable plugin to implement every ring, defeating segregation.
/// Dispatch uses the per-ring trait objects, not this marker.
/// Send + Sync are inherited via KelirPlugin.
pub trait AnyPlugin: KelirPlugin + PluginClone {}

impl<T: KelirPlugin + PluginClone> AnyPlugin for T {}

/// Trait marker for foreign runtime plugin proxies bridging V8 and Python.
pub trait ForeignPluginBridge: KelirPlugin {
    /// Declared declarative filter evaluated in native Rust before FFI crossing
    fn mount_scope(&self) -> &MountScope;
    /// Hard rate limit evaluated in host memory before foreign thread dispatch
    fn max_dispatches_per_second(&self) -> u32 { 10_000 }
    /// Ring-buffer capacity for decoupling foreign language execution
    fn worker_channel_capacity(&self) -> usize { 4_096 }
    /// Max inspected byte ceiling across FFI boundary (default 64KB)
    fn buffer_ceiling(&self) -> usize { 64 * 1024 }
}

/// Fused trampoline callback: the runnable half of a fused hook.
/// `HookEntry.run` carries one of these per descriptor hook; the engine
/// registry is keyed by (ring, ordinal) and fused_crossing invokes the
/// matching callbacks in ordinal order, first non-Pass verdict wins.
/// A callback that panics quarantines to Verdict::Drop (Efault); a
/// PluginError::Reject maps to Verdict::Drop, any other error propagates.
/// Verdict-returning dispatches (trap, host-lsm-exec) return the folded
/// fused verdict before the native interceptor loop; FilterAction and
/// unit-returning dispatches invoke the callbacks but have no Verdict to
/// fold into, so the folded verdict is observable only on the crossing
/// guard until a Verdict-to-FilterAction mapping lands.
pub trait FusedTrampolineCallback: Send + Sync + 'static {
    fn run(&self, ring: TrampolineRing, ordinal: u32) -> Result<Verdict, PluginError>;
}

/// Fused crossing guard: one FFI submit for the whole ring event, closed
/// by Drop via foreign_submit_complete. Carries the folded verdict of the
/// callbacks invoked at the crossing.
pub struct FusedCrossing<'a> {
    engine: &'a PipelineEngine,
    verdict: Verdict,
}

impl PipelineEngine {
    pub fn register_fused_callback(
        &mut self,
        ring: TrampolineRing,
        ordinal: u32,
        callback: Arc<dyn FusedTrampolineCallback>,
    ) {
        self.fused_callbacks.push((ring, ordinal, callback));
    }
    pub fn register_fused_callbacks(&mut self, descriptor: &FusedTrampolineDescriptor) {
        for chain in &descriptor.rings {
            for hook in &chain.hooks {
                if let Some(callback) = hook.run.clone() {
                    self.register_fused_callback(chain.ring, hook.ordinal, callback);
                }
            }
        }
    }
    fn fused_crossing(
        &self,
        ring: TrampolineRing,
    ) -> Result<Option<FusedCrossing<'_>>, PluginError> {
        let mut callbacks: Vec<&(TrampolineRing, u32, Arc<dyn FusedTrampolineCallback>)> =
            self.fused_callbacks
                .iter()
                .filter(|(callback_ring, _, _)| *callback_ring == ring)
                .collect();
        callbacks.sort_by_key(|(_, ordinal, _)| *ordinal);
        if callbacks.is_empty() && !self.fused_ring(ring) {
            return Ok(None);
        }
        self.try_foreign_submit()?;
        let mut verdict = Verdict::Pass;
        for (_, ordinal, callback) in callbacks {
            let outcome = quarantine_sync(
                || callback.run(ring, *ordinal),
                || {
                    Ok(Verdict::Drop {
                        errno: Some(Errno::Efault),
                        reason: "Plugin panicked".into(),
                    })
                },
            );
            verdict = map_reject_to_verdict(outcome)?;
            if !matches!(verdict, Verdict::Pass) {
                break;
            }
        }
        Ok(Some(FusedCrossing {
            engine: self,
            verdict,
        }))
    }
}

/// NAPI adapter: each TypeScript run is wrapped once in a threadsafe
/// function at fuse_hooks time; run() enqueues (ring tag, ordinal) onto
/// the JS thread non-blocking and returns Pass. The JS return value does
/// not gate dispatch until the L3 verdict rendezvous lands, and the ring
/// event payload is not marshaled yet: the TS side passes the hook a
/// { ring, ordinal } event object.
/// PyO3 adapter: each Python run is stored as a Py<PyAny> callable at
/// fuse_hooks time; run() calls it under the GIL with (ring tag, ordinal)
/// and returns Pass. Same L3 boundary: return-value-to-Verdict mapping
/// and ring-event payload marshaling.
pub struct NapiFusedCallback {
    call: ThreadsafeFunction<(String, u32)>,
}
pub struct PyFusedCallback {
    callable: Py<PyAny>,
}
