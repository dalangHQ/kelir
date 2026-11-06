// see: cans/02-plugin-model.md#KelirPlugin-trait
// see: cans/02-plugin-model.md#Segregated-ring-traits
use std::any::Any;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::driver::{WatchdogAction, WatchdogEvent, WatchdogVerdict};
use crate::errors::PluginError;
use crate::protocol::{FilterAction, ProcessExit, TrapEvent, Verdict};
use crate::telemetry::{ExecutionReceipt, ExitKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtioDevice {
    Vsock,
    Console,
    Fs,
    Net,
    Shm,
    Auxiliary,
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
    Vsock,
    Console,
    Fs,
    Net,
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

/// Quarantine boundary guarantee: every interposer dispatch is executed
/// within std::panic::catch_unwind at the supervisor engine perimeter.
/// A panicking plugin does not unwind across the FFI or crash the microkernel;
/// the engine converts the panic into Verdict::Drop { errno: Some("EFAULT"),
/// reason: "Plugin panicked" } or PluginError::Halt, logs the backtrace
/// to the host ledger, and rolls back the active StepSlot.
pub const PANIC_QUARANTINE_ENABLED: bool = true;

// NOTE: order's MountScope fields (allowed_rings/path_subtrees/cidr_allowlists/binary_paths) dropped; artifact shape (devices/binaries/paths) wins per cans/artifacts/code/plugin-trait.rs.
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

    pub fn with_binaries<I: IntoIterator<Item = S>, S: Into<String>>(
        mut self,
        binaries: I,
    ) -> Self {
        self.binaries = Some(binaries.into_iter().map(Into::into).collect());
        self
    }

    pub fn with_paths<I: IntoIterator<Item = P>, P: Into<PathBuf>>(mut self, paths: I) -> Self {
        self.paths = Some(paths.into_iter().map(Into::into).collect());
        self
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VetoEvent {
    Fork,
    Suspend,
    ClockSkew,
}

/// Observation-only lifecycle events. Published via EventBusBinder
/// as non-blocking telemetry. Not vetoable; no return value.
#[derive(Debug, Clone, Copy)]
pub enum LifecycleObservation {
    Checkpoint,
    Rollback,
    Resume,
    Terminate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleEvent {
    VmInit,
    VmReady,
    VmPreempt,
    VmShutdown,
}

#[derive(Debug, Clone)]
pub struct HandlerCtx {
    pub session_id: [u8; 16],
    pub pid: u32,
}

impl HandlerCtx {
    pub fn new(session_id: [u8; 16], pid: u32) -> Self {
        Self { session_id, pid }
    }

    pub fn receipt(&self) -> ExecutionReceipt {
        // TODO: wire to the live per-process receipt in Part 2.
        ExecutionReceipt {
            pid: self.pid,
            binary_path: String::new(),
            binary_blake3: String::new(),
            exit_kind: ExitKind::Normal,
            correlation_token: None,
            dirty_bytes: 0,
            syscall_count: None,
            exit_code: 0,
            wall_clock_ns: 0,
            phase_timings: None,
            egress_bytes: 0,
            egress_dropped_bytes: 0,
            extensions: Vec::new(),
        }
    }
}

pub struct LifecycleCtx {
    pub watchdog: Option<WatchdogAction>,
    pub balloon_target_mb: Option<u32>,
    pub vetoes: Vec<(VetoEvent, Box<dyn Fn() -> bool + Send + Sync>)>,
}

impl LifecycleCtx {
    pub fn new() -> Self {
        Self {
            watchdog: None,
            balloon_target_mb: None,
            vetoes: Vec::new(),
        }
    }

    pub fn set_watchdog(&mut self, action: WatchdogAction) -> Result<(), PluginError> {
        self.watchdog = Some(action);
        Ok(())
    }

    pub fn adjust_balloon(&mut self, target_mb: u32) -> Result<(), PluginError> {
        self.balloon_target_mb = Some(target_mb);
        Ok(())
    }

    pub fn register_veto(
        &mut self,
        event: VetoEvent,
        callback: Box<dyn Fn() -> bool + Send + Sync>,
    ) -> Result<(), PluginError> {
        self.vetoes.push((event, callback));
        Ok(())
    }
}

impl Default for LifecycleCtx {
    fn default() -> Self {
        Self::new()
    }
}

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

pub trait AnyPlugin: KelirPlugin + PluginClone {}

impl<T: KelirPlugin + PluginClone> AnyPlugin for T {}

/// Binds inline interceptors into the linear monotonic pipeline.
/// No service registry: peer capabilities are typed Context values
/// accumulated into the builder's Plugins tuple (see KelirPlugin::Context).
pub trait PipelineBinder {
    fn bind_fs(&mut self, stage: Arc<dyn FsInterposer>);
    fn bind_net(&mut self, stage: Arc<dyn NetInterposer>);
    fn bind_trap(&mut self, stage: Arc<dyn TrapInterposer>);
    fn bind_console(&mut self, stage: Arc<dyn ConsoleInterposer>);
    fn bind_host(&mut self, stage: Arc<dyn HostInterposer>);
}

/// Binds non-blocking observers to the immutable telemetry bus.
pub trait EventBusBinder {
    fn subscribe<E: Any + Send + Sync + 'static>(&mut self, handler: Box<dyn Fn(&E) + Send + Sync>);
}

/// Assembly-time composite plugin tree unpacked into flat dispatch entries before boot.
pub trait CompositePlugin: Send + Sync {
    fn unpack(self: Box<Self>, parent_scope: &MountScope) -> Vec<(Box<dyn AnyPlugin>, MountScope)>;
}

// TODO (L2): async hooks + Context/register_* per cans/artifacts/code/plugin-trait.rs — sync L1 contract here.
pub trait KelirPlugin: Send + Sync + PluginClone {
    fn name(&self) -> &'static str;
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
    fn is_enabled(&self) -> bool {
        true
    }
    fn fork_state(&self, _child_session_id: &str) -> Box<dyn AnyPlugin> {
        self.clone_box()
    }
}

pub trait LifecycleParticipant: Send + Sync {
    fn on_vm_init(&self, ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        let _ = ctx;
        Ok(())
    }
    fn on_vm_ready(&self, ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        let _ = ctx;
        Ok(())
    }
    fn on_vm_shutdown(&self, ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        let _ = ctx;
        Ok(())
    }
    // NOTE: artifact on_pre_commit takes (&mut LifecycleCtx); sync L1 drops the ctx per work-order Task 1.5.
    fn on_pre_commit(&self) -> Result<(), PluginError> {
        Ok(())
    }
    fn on_post_commit(&self, success: bool) -> Result<(), PluginError> {
        let _ = success;
        Ok(())
    }
    fn on_host_preempt(&self) -> Result<(), PluginError> {
        Ok(())
    }
}

// NOTE: hooks are sync here; artifact defines them async under #[async_trait] per cans/artifacts/code/plugin-trait.rs.
pub trait TrapInterposer: Send + Sync {
    fn on_trap(&self, event: &TrapEvent, ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        let _ = (event, ctx);
        Ok(Verdict::Pass)
    }
    fn on_process_exit(&self, event: &ProcessExit, ctx: &HandlerCtx) -> Result<(), PluginError> {
        let _ = (event, ctx);
        Ok(())
    }
}

pub trait ConsoleInterposer: Send + Sync {
    fn on_console_input(&self, port: u32, bytes: &mut Vec<u8>) -> Result<(), PluginError> {
        let _ = (port, bytes);
        Ok(())
    }
    fn on_console_output(&self, port: u32, chunk: &mut Vec<u8>) -> Result<(), PluginError> {
        let _ = (port, chunk);
        Ok(())
    }
    fn on_exec_stdout(
        &self,
        pid: u32,
        exec_id: u64,
        chunk: &mut Vec<u8>,
    ) -> Result<(), PluginError> {
        let _ = (pid, exec_id, chunk);
        Ok(())
    }
    fn on_exec_stderr(
        &self,
        pid: u32,
        exec_id: u64,
        chunk: &mut Vec<u8>,
    ) -> Result<(), PluginError> {
        let _ = (pid, exec_id, chunk);
        Ok(())
    }
}

pub trait SystemTelemetryParticipant: Send + Sync {
    fn on_memory_pressure(&self, available_mb: u32, total_mb: u32) -> Result<(), PluginError> {
        let _ = (available_mb, total_mb);
        Ok(())
    }
    fn on_heartbeat(&self, uptime_ms: u64) -> Result<(), PluginError> {
        let _ = uptime_ms;
        Ok(())
    }
    fn on_watchdog(&self, event: &WatchdogEvent) -> Result<WatchdogVerdict, PluginError> {
        let _ = event;
        Ok(WatchdogVerdict::ExecuteDefault)
    }
}

pub trait FsInterposer: Send + Sync {
    fn on_fs_lookup(&self, path: &Path) -> Result<Verdict, PluginError> {
        let _ = path;
        Ok(Verdict::Pass)
    }
    fn on_fs_read(&self, path: &Path, buffer: &mut [u8]) -> Result<(), PluginError> {
        let _ = (path, buffer);
        Ok(())
    }
    fn on_fs_write(
        &self,
        path: &Path,
        data: &mut [u8],
        pid: u32,
        ctx: &HandlerCtx,
    ) -> Result<FilterAction, PluginError> {
        let _ = (path, data, pid, ctx);
        Ok(FilterAction::Forward)
    }
    fn on_fs_lock(
        &self,
        path: &Path,
        flock_cmd: i32,
        flock_type: i16,
        pid: u32,
    ) -> Result<Verdict, PluginError> {
        let _ = (path, flock_cmd, flock_type, pid);
        Ok(Verdict::Pass)
    }
}

pub trait NetInterposer: Send + Sync {
    fn on_net_outbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        let _ = frame;
        Ok(FilterAction::Forward)
    }
    fn on_net_inbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        let _ = frame;
        Ok(FilterAction::Forward)
    }
}

pub trait HostInterposer: Send + Sync {
    fn on_host_lsm_exec(&self, path: &Path, pid: u32) -> Result<Verdict, PluginError> {
        let _ = (path, pid);
        Ok(Verdict::Pass)
    }
    fn on_host_pressure(&self, level: u32) -> Result<Verdict, PluginError> {
        let _ = level;
        Ok(Verdict::Pass)
    }
    fn on_host_keyring_access(&self, key: &str) -> Result<Verdict, PluginError> {
        let _ = key;
        Ok(Verdict::Pass)
    }
}

/// Trait marker for foreign runtime plugin proxies bridging V8 and Python.
pub trait ForeignPluginBridge: KelirPlugin {
    /// Declared declarative filter evaluated in native Rust before FFI crossing
    fn mount_scope(&self) -> &MountScope;
    /// Hard rate limit evaluated in host memory before foreign thread dispatch
    fn max_dispatches_per_second(&self) -> u32 {
        10_000
    }
    /// Ring-buffer capacity for decoupling foreign language execution
    fn worker_channel_capacity(&self) -> usize {
        4_096
    }
    /// Max inspected byte ceiling across FFI boundary (default 64KB)
    fn buffer_ceiling(&self) -> usize {
        64 * 1024
    }
}
