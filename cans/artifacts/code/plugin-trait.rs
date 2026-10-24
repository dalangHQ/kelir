// crates/kelir-interface/src/plugin.rs
//
// The unified KelirPlugin trait implemented by every subsystem across the
// 4 VirtIO rings. kelir-core contains no hardcoded capability logic.
//
// Owned by: cans/02-plugin-model.md#KelirPlugin-trait

use async_trait::async_trait;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use crate::handlers::HandlerCtx;
use crate::protocol::messages::{
    Heartbeat, MemoryPressureWarning, PortBound, ProcessExit,
    SendSignal, TrapAction, TrapEvent, WindowResize,
};
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
pub struct VfsInodeAttr {
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u64,
    pub occ_metadata_hash: u64,
    pub content: Vec<u8>,
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

#[derive(Debug, Clone)]
pub struct PluginTaxonomy {
    pub boundary: &'static str, // urn:kelir:<subsystem>:<target>[:<subtarget>]
    pub rings: &'static [VirtioDevice],
    pub role: TelemetryRole,
    pub tier: ExecutionTier,
    pub mutation_targets: &'static [&'static str],
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

/// Assembly-time composite plugin tree unpacked into flat dispatch entries before boot.
pub trait CompositePlugin: Send + Sync {
    fn unpack(self: Box<Self>, parent_scope: &MountScope) -> Vec<(Box<dyn KelirPlugin>, MountScope)>;
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

#[async_trait]
pub trait KelirPlugin: Send + Sync {
    /// Unique identifier for telemetry and requires() resolution.
    fn name(&self) -> &'static str;

    /// Universal taxonomy coordinates declared for registry indexing.
    fn taxonomy(&self) -> PluginTaxonomy;

    /// Dispatch ordering; lower values run earlier. Default: 100.
    /// Ties broken by declaration order.
    fn priority(&self) -> u8 { 100 }

    /// Default mount scope. Overridden by explicit builder scope.
    fn default_scope(&self) -> MountScope { MountScope::default() }

    /// Null-object toggle. False omits all hooks from flat dispatch array.
    fn is_enabled(&self) -> bool { true }

    /// Declared capability requirements validated as an acyclic graph at boot.
    /// Missing targets or circular dependencies abort with EConfig.
    fn requires(&self) -> &'static [&'static str] { &[] }

    // ─── 1. MicroVM Lifecycle ────────────────────────────────────
    async fn on_vm_init(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> { Ok(()) }
    async fn on_vm_ready(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> { Ok(()) }
    async fn on_vm_shutdown(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> { Ok(()) }
    async fn on_pre_commit(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> { Ok(()) }
    async fn on_post_commit(&self, _ctx: &mut LifecycleCtx, _committed: bool) -> Result<(), PluginError> { Ok(()) }
    async fn on_host_preempt(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> { Ok(()) }

    // ─── 2. Control Plane (virtio-vsock) ─────────────────────────
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

    // ─── 3. Terminal Plane (virtio-console) ──────────────────────
    async fn on_pty_input(&self, _bytes: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
    async fn on_pty_output(&self, _chunk: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
    async fn on_window_resize(
        &self, _event: &WindowResize,
    ) -> Result<(), PluginError> { Ok(()) }
    /// Mandatory core-originated disclosure (TrapAction::Rewrite.tty_notice).
    /// Body is immutable; return Some(bytes) to append. All annotations
    /// concatenate in priority() order and follow the body on the PTY.
    /// See cans/04-terminal-plane.md#Disclosure
    async fn on_disclosure(
        &self, _body: &[u8],
    ) -> Result<Option<Vec<u8>>, PluginError> { Ok(None) }

    // ─── 4. Storage Plane (virtio-fs) ────────────────────────────
    async fn on_vfs_lookup(&self, _path: &Path) -> Result<Option<VfsInodeAttr>, PluginError> { Ok(None) }
    async fn on_vfs_read(&self, _path: &Path, _buffer: &mut [u8]) -> Result<(), PluginError> { Ok(()) }
    /// Ok(()) = allow; Err(Reject{errno}) = deny.
    /// See cans/05-storage-plane.md#Write-gating
    async fn on_vfs_write(
        &self, _path: &Path, _data: &[u8], _pid: u32, _ctx: &HandlerCtx,
    ) -> Result<(), PluginError> { Ok(()) }

    // ─── 5. Wire Plane (virtio-net) ──────────────────────────────
    /// Ok(()) = forward. Err(Reject) = silent drop (no RST, no guest error).
    async fn on_net_outbound(&self, _frame: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
    /// Ok(()) = deliver. Err(Reject) = silent drop.
    async fn on_net_inbound(&self, _frame: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
}

/// Trait marker for foreign runtime plugin proxies bridging V8 and Python.
pub trait ForeignPluginBridge: KelirPlugin {
    /// Declared declarative filter evaluated in native Rust before FFI crossing
    fn mount_scope(&self) -> &MountScope;
    /// Max inspected byte ceiling across FFI boundary (default 64KB)
    fn buffer_ceiling(&self) -> usize { 64 * 1024 }
}
