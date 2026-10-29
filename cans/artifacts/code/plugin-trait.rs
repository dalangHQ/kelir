// crates/kelir-interface/src/plugin.rs
//
// The typed KelirPlugin trait and ring interposer contracts.
// Plugins link directly via Cargo crates; no runtime string resolution.
//
// Owned by: cans/02-plugin-model.md#KelirPlugin-trait

use async_trait::async_trait;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::any::{Any, TypeId};
use std::sync::Arc;
use crate::handlers::HandlerCtx;
use crate::protocol::messages::{
    Heartbeat, MemoryPressureWarning, PortBound, ProcessExit,
    TrapAction, TrapEvent,
};

// ── Decoded hook payloads ────────────────────────────────────────
// Wire transport for terminal control and signals is ChannelData
// (see cans/09-protocol.md#Wire-messages); these structs are the
// decoded payloads delivered to TrapInterposer/ConsoleInterposer
// hooks, not wire variants. Field layouts restored verbatim from
// the pre-purge messages.rs definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowResize {
    pub rows: u16,
    pub cols: u16,
    pub xpixel: u16,
    pub ypixel: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendSignal {
    pub pid: u32,
    pub signal: u32,
}
use crate::vmm::driver::{VmmDriver, VmConfig, WatchdogAction};

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    /// Hard failure; halts the pipeline and surfaces to the runtime.
    #[error("Plugin execution rejected: {0}")]
    Halt(String),
    /// Deny with a POSIX errno name surfaced to the guest.
    /// Storage plane: guest write() fails with this errno.
    /// Wire plane:    frame dropped silently (errno for logging only).
    #[error("Plugin rejected: {errno} ({reason})")]
    Reject { errno: String, reason: String },
    /// Payload size exceeds the foreign FFI boundary ceiling.
    #[error("Inspection buffer ceiling exceeded: {size} bytes (max {max_bytes})")]
    BufferCeilingExceeded { size: usize, max_bytes: usize },
    /// Host event loop or foreign runtime thread starvation detected.
    #[error("Foreign FFI channel saturated: worker queue depth exceeded ({depth})")]
    FfiQueueSaturated { depth: usize },
    /// Trap violation against W^X memory execution invariants.
    #[error("W^X execution violation detected: PID {pid} attempted {operation}")]
    WxViolation { pid: u32, operation: String },
    /// Conflicting semantic mutations on overlapping resource targets.
    #[error("Ambiguous mutation detected across plugins on resource '{resource}': {conflict}")]
    AmbiguousMutation { resource: String, conflict: String },
    #[error("Internal I/O failure: {0}")]
    Io(#[from] std::io::Error),
}

impl PluginError {
    pub fn ambiguous_mutation(
        resource: impl Into<String>,
        conflict: impl Into<String>,
    ) -> Self {
        Self::AmbiguousMutation { resource: resource.into(), conflict: conflict.into() }
    }
    pub fn reject(errno: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Reject { errno: errno.into(), reason: reason.into() }
    }
}

/// Complete inode attribute set surfaced by storage-plane lookups.
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
#[derive(Debug, Clone, Default)]
pub struct MountScope {
    pub devices: Option<Vec<VirtioDevice>>,
    pub binaries: Option<Vec<String>>,
    pub paths: Option<Vec<PathBuf>>,
}

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

/// Synchronous verdict on hot-path VirtIO rings (Fs, Net).
/// Eliminates async allocation and libuv/asyncio event loop overhead on the fast path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncVerdict {
    Allow,
    YieldToHost(u64),
}

/// Strongly-typed service locator eliminating ambient context string bags.
pub trait ServiceRegistry: Send + Sync {
    fn register_service<T: ?Sized + Any + Send + Sync>(&mut self, service: Arc<T>);
    fn get_service<T: ?Sized + Any + Send + Sync>(&self) -> Option<Arc<T>>;
}

/// Binds inline interceptors into the linear monotonic pipeline.
pub trait PipelineBinder: ServiceRegistry {
    fn bind_fs(&mut self, stage: Arc<dyn FsInterposer>);
    fn bind_net(&mut self, stage: Arc<dyn NetInterposer>);
    fn bind_trap(&mut self, stage: Arc<dyn TrapInterposer>);
    fn bind_console(&mut self, stage: Arc<dyn ConsoleInterposer>);
}

/// Binds non-blocking observers to the immutable telemetry bus.
pub trait EventBusBinder {
    fn subscribe<E: Any + Send + Sync + 'static>(&mut self, handler: Box<dyn Fn(&E) + Send + Sync>);
}

/// Assembly-time composite plugin tree unpacked into flat dispatch entries before boot.
pub trait CompositePlugin: Send + Sync {
    fn unpack(self: Box<Self>, parent_scope: &MountScope) -> Vec<(Box<dyn AnyPlugin>, MountScope)>;
}

// ── Lifecycle context ────────────────────────────────────────────
// Restricted handle for on_vm_init. Exposes only mutating capabilities
// that are pre-boot config; never the raw &mut dyn VmmDriver.

#[derive(Debug, Clone, Copy)]
pub enum VetoEvent {
    Fork, Checkpoint, Rollback, Suspend, Resume,
    Terminate, ClockSkew,
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
    vetoes: &'a mut Vec<(VetoEvent, Box<dyn Fn() -> bool + Send + Sync>)>,
}

impl<'a> LifecycleCtx<'a> {
    pub async fn set_watchdog(
        &mut self, timeout: Duration, action: WatchdogAction,
    ) -> Result<(), PluginError> {
        self.driver.set_watchdog(timeout, action).await
            .map_err(PluginError::Io)
    }
    pub async fn adjust_balloon(&mut self, target_mb: u32) -> Result<(), PluginError> {
        self.driver.adjust_balloon(target_mb).await
            .map_err(PluginError::Io)
    }
    /// Register a veto callback for a lifecycle action. All registered
    /// callbacks run before the action; any returning false aborts it.
    pub fn register_veto(
        &mut self, event: VetoEvent,
        f: Box<dyn Fn() -> bool + Send + Sync>,
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
    /// Unique identifier for telemetry.
    fn name(&self) -> &'static str;

    /// Default mount scope. Overridden by explicit builder scope.
    fn default_scope(&self) -> MountScope { MountScope::default() }

    /// Null-object toggle. False omits all hooks from flat dispatch array.
    fn is_enabled(&self) -> bool { true }

    /// Deep-clones plugin state when session.vm.fork() creates a CoW child.
    fn fork_state(&self, child_session_id: &str) -> Box<dyn AnyPlugin>;

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
    ) -> Result<Option<TrapAction>, PluginError> { Ok(None) }
    async fn on_process_exit(
        &self, _event: &ProcessExit, _ctx: &HandlerCtx,
    ) -> Result<(), PluginError> { Ok(()) }
    async fn on_memory_pressure(
        &self, _event: &MemoryPressureWarning,
    ) -> Result<(), PluginError> { Ok(()) }
    async fn on_heartbeat(
        &self, _event: &Heartbeat,
    ) -> Result<(), PluginError> { Ok(()) }
    async fn on_port_bound(
        &self, _event: &PortBound,
    ) -> Result<(), PluginError> { Ok(()) }
    async fn on_signal_send(
        &self, _event: &SendSignal,
    ) -> Result<(), PluginError> { Ok(()) }
    async fn on_watchdog(
        &self, _event: &WatchdogEvent,
    ) -> Result<WatchdogVerdict, PluginError> { Ok(WatchdogVerdict::ExecuteDefault) }
}

#[async_trait]
pub trait ConsoleInterposer {
    async fn on_console_input(&self, _bytes: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
    async fn on_console_output(&self, _chunk: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
    async fn on_window_resize(
        &self, _event: &WindowResize,
    ) -> Result<(), PluginError> { Ok(()) }
    /// Mandatory core-originated disclosure (HostToGuest::ChannelData on the pty-disclosure kind).
    /// Body is immutable; return Some(bytes) to append. All annotations
    /// concatenate in registration order and follow the body on the PTY.
    /// See cans/04-terminal-plane.md#Disclosure
    async fn on_disclosure(
        &self, _body: &[u8],
    ) -> Result<Option<Vec<u8>>, PluginError> { Ok(None) }
}

pub trait FsInterposer {
    fn on_fs_lookup(&self, _path: &Path) -> Result<Option<FsInodeAttr>, PluginError> { Ok(None) }
    fn on_fs_read(&self, _path: &Path, _buffer: &mut [u8]) -> Result<(), PluginError> { Ok(()) }
    /// Synchronous hot-path inspection. Returns Ok(Allow) or Ok(YieldToHost) before write applies.
    fn on_fs_write(
        &self, _path: &Path, _data: &[u8], _pid: u32, _ctx: &HandlerCtx,
    ) -> Result<SyncVerdict, PluginError> { Ok(SyncVerdict::Allow) }
}

pub trait NetInterposer {
    /// Synchronous L2 outbound inspection.
    fn on_net_outbound(&self, _frame: &mut [u8]) -> Result<SyncVerdict, PluginError> { Ok(SyncVerdict::Allow) }
    /// Synchronous L2 inbound inspection.
    fn on_net_inbound(&self, _frame: &mut [u8]) -> Result<SyncVerdict, PluginError> { Ok(SyncVerdict::Allow) }
}

/// Identity marker for any plugin object: core identity only.
/// fork_state and CompositePlugin::unpack return this type. It must NOT
/// require the ring traits: requiring all five would force every
/// forkable plugin to implement every ring, defeating segregation.
/// Dispatch uses the per-ring trait objects, not this marker.
/// Send + Sync are inherited via KelirPlugin.
pub trait AnyPlugin: KelirPlugin {}

impl<T: KelirPlugin> AnyPlugin for T {}

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
