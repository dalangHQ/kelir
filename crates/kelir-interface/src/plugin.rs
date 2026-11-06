// see: cans/02-plugin-model.md#KelirPlugin-trait
// see: cans/02-plugin-model.md#Segregated-ring-traits
use std::any::Any;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use thiserror::Error;

use crate::service_registry::ServiceRegistry;
use crate::vsock::{VsockServiceHandler, VsockServiceSpec};

use crate::driver::{
    BalloonConvergence, ClockStepControl, EntropyBackendControl, WatchdogAction, WatchdogEvent,
    WatchdogVerdict,
};
pub use crate::handlers::{HandlerCtx, Has};
use crate::protocol::{FilterAction, ProcessExit, StreamDirection, StreamKind, TrapEvent, Verdict};

// see: cans/artifacts/code/plugin-trait.rs
#[derive(Error, Debug)]
pub enum PluginError {
    #[error("Foreign FFI channel saturated: worker queue depth exceeded ({depth})")]
    FfiQueueSaturated { depth: usize },
    #[error("W^X execution violation detected: PID {pid} attempted {operation}")]
    WxViolation { pid: u32, operation: String },
    #[error("Ambiguous mutation detected across plugins on resource '{resource}': {conflict}")]
    AmbiguousMutation { resource: String, conflict: String },
    #[error("Internal I/O failure: {0}")]
    Io(#[from] std::io::Error),
    #[error("Plugin halted: {0}")]
    Halt(String),
}

impl PluginError {
    pub fn ambiguous_mutation(resource: impl Into<String>, conflict: impl Into<String>) -> Self {
        Self::AmbiguousMutation {
            resource: resource.into(),
            conflict: conflict.into(),
        }
    }

    pub fn reject(errno: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Halt(format!("{}: {}", errno.into(), reason.into()))
    }
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

pub const PANIC_QUARANTINE_ENABLED: bool = true;

// see: cans/artifacts/code/plugin-trait.rs
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

// see: cans/plugins/transaction.md
#[async_trait]
pub trait VcpuFreezeControl: Send + Sync {
    async fn freeze(&self) -> Result<(), PluginError>;
    async fn unfreeze(&self) -> Result<(), PluginError>;
}

// see: cans/plugins/transaction.md
#[async_trait]
pub trait FuseDrainControl: Send + Sync {
    async fn drain_in_flight(&self, timeout: Duration) -> Result<(), PluginError>;
}

// see: cans/plugins/transaction.md
#[async_trait]
pub trait CowBrokerControl: Send + Sync {
    async fn commit_upper(&self) -> Result<(), PluginError>;
    async fn discard_upper(&self) -> Result<(), PluginError>;
}

// TODO (L3): borrow &mut dyn VmmDriver + &VmConfig per cans/artifacts/code/plugin-trait.rs; owned store is the L2 stand-in.
pub struct LifecycleCtx {
    pub watchdog: Option<WatchdogAction>,
    pub watchdog_timeout: Option<Duration>,
    pub balloon_target_mb: Option<u32>,
    pub vetoes: Vec<(VetoEvent, Box<dyn Fn() -> bool + Send + Sync>)>,
    pub vcpu_freeze: Option<Arc<dyn VcpuFreezeControl>>,
    pub fuse_drain: Option<Arc<dyn FuseDrainControl>>,
    pub cow_broker: Option<Arc<dyn CowBrokerControl>>,
    pub clock_step: Option<Arc<dyn ClockStepControl>>,
    pub entropy: Option<Arc<dyn EntropyBackendControl>>,
}

impl LifecycleCtx {
    pub fn new() -> Self {
        Self {
            watchdog: None,
            watchdog_timeout: None,
            balloon_target_mb: None,
            vetoes: Vec::new(),
            vcpu_freeze: None,
            fuse_drain: None,
            cow_broker: None,
            clock_step: None,
            entropy: None,
        }
    }

    pub async fn set_watchdog(
        &mut self,
        timeout: Duration,
        action: WatchdogAction,
    ) -> Result<(), PluginError> {
        self.watchdog_timeout = Some(timeout);
        self.watchdog = Some(action);
        Ok(())
    }

    pub async fn adjust_balloon(
        &mut self,
        target_mb: u32,
    ) -> Result<BalloonConvergence, PluginError> {
        // TODO (L3): converge via driver balloon ack per cans/artifacts/code/vmm-driver-trait.rs.
        self.balloon_target_mb = Some(target_mb);
        Ok(BalloonConvergence {
            target_mb,
            converged: false,
        })
    }

    pub fn register_veto(
        &mut self,
        event: VetoEvent,
        callback: Box<dyn Fn() -> bool + Send + Sync>,
    ) {
        self.vetoes.push((event, callback));
    }

    pub async fn freeze_vcpus(&self) -> Result<(), PluginError> {
        match &self.vcpu_freeze {
            Some(control) => control.freeze().await,
            None => Err(PluginError::Halt("vCPU freeze capability not wired".into())),
        }
    }

    pub async fn unfreeze_vcpus(&self) -> Result<(), PluginError> {
        match &self.vcpu_freeze {
            Some(control) => control.unfreeze().await,
            None => Err(PluginError::Halt("vCPU freeze capability not wired".into())),
        }
    }

    pub async fn drain_fuse(&self, timeout: Duration) -> Result<(), PluginError> {
        match &self.fuse_drain {
            Some(control) => control.drain_in_flight(timeout).await,
            None => Err(PluginError::Halt("FUSE drain capability not wired".into())),
        }
    }

    pub async fn commit_broker_upper(&self) -> Result<(), PluginError> {
        match &self.cow_broker {
            Some(control) => control.commit_upper().await,
            None => Err(PluginError::Halt("CoW broker capability not wired".into())),
        }
    }

    pub async fn discard_broker_upper(&self) -> Result<(), PluginError> {
        match &self.cow_broker {
            Some(control) => control.discard_upper().await,
            None => Err(PluginError::Halt("CoW broker capability not wired".into())),
        }
    }

    pub fn clock_step_control(&self) -> Result<Arc<dyn ClockStepControl>, PluginError> {
        match &self.clock_step {
            Some(control) => Ok(Arc::clone(control)),
            None => Err(PluginError::Halt(
                "clock stepping capability not wired".into(),
            )),
        }
    }

    pub fn entropy_control(&self) -> Result<Arc<dyn EntropyBackendControl>, PluginError> {
        match &self.entropy {
            Some(control) => Ok(Arc::clone(control)),
            None => Err(PluginError::Halt(
                "entropy backend capability not wired".into(),
            )),
        }
    }

    pub fn update_dispatch(&mut self, _new_dispatch: Arc<()>) -> Result<(), PluginError> {
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

pub trait AnyPlugin: Send + Sync {
    fn name(&self) -> &'static str;
    fn fork_state(&self, _child_session_id: &str) -> Box<dyn AnyPlugin>;
}

impl<T> AnyPlugin for T
where
    T: 'static + KelirPlugin + Clone,
{
    fn name(&self) -> &'static str {
        KelirPlugin::name(self)
    }

    fn fork_state(&self, child_session_id: &str) -> Box<dyn AnyPlugin> {
        KelirPlugin::fork_state(self, child_session_id)
    }
}

// see: cans/11-protocol.md#Control-channel
// see: cans/plugins/db-proxy.md
pub trait PipelineBinder {
    fn bind_fs(&mut self, stage: Arc<dyn FsInterposer>);
    fn bind_net(&mut self, stage: Arc<dyn NetInterposer>);
    fn bind_trap(&mut self, stage: Arc<dyn TrapInterposer>);
    fn bind_console(&mut self, stage: Arc<dyn ConsoleInterposer>);
    fn bind_host(&mut self, stage: Arc<dyn HostInterposer>);
    fn bind_stream(&mut self, kind: StreamKind, stage: Arc<dyn StreamInterposer>);
    fn bind_vsock_service(&mut self, spec: VsockServiceSpec, service: Arc<dyn VsockServiceHandler>);
    fn service_registry(&mut self) -> &mut ServiceRegistry;
}

pub trait EventBusBinder {
    fn subscribe<E: Any + Send + Sync + 'static>(&mut self, handler: Box<dyn Fn(&E) + Send + Sync>);
}

pub trait CompositePlugin: Send + Sync {
    fn unpack(self: Box<Self>, parent_scope: &MountScope) -> Vec<(Box<dyn AnyPlugin>, MountScope)>;
}

// see: cans/artifacts/code/plugin-trait.rs
// TODO (L3): cans/artifacts/code/plugin-trait.rs writes &mut dyn EventBusBinder, but subscribe is generic so the trait is not object-safe; &mut impl is the compiling form.
pub trait KelirPlugin: Send + Sync + PluginClone {
    type Context: Send + Sync + Clone + 'static;
    fn context(&self) -> Self::Context;
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
    fn register_interceptors(&self, _pipeline: &mut dyn PipelineBinder) {}
    fn register_subscribers(&self, _bus: &mut impl EventBusBinder) {}
}

#[async_trait]
pub trait LifecycleParticipant: Send + Sync {
    async fn on_vm_init(&self, ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        let _ = ctx;
        Ok(())
    }
    async fn on_vm_ready(&self, ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        let _ = ctx;
        Ok(())
    }
    async fn on_vm_shutdown(&self, ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        let _ = ctx;
        Ok(())
    }
    async fn on_pre_commit(&self, ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        let _ = ctx;
        Ok(())
    }
    async fn on_post_commit(
        &self,
        ctx: &mut LifecycleCtx,
        committed: bool,
    ) -> Result<(), PluginError> {
        let _ = (ctx, committed);
        Ok(())
    }
    async fn on_host_preempt(&self, ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        let _ = ctx;
        Ok(())
    }
}

// see: cans/artifacts/code/plugin-trait.rs
#[async_trait]
pub trait TrapInterposer: Send + Sync {
    async fn on_trap(&self, event: &TrapEvent, ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        let _ = (event, ctx);
        Ok(Verdict::Pass)
    }
    async fn on_process_exit(
        &self,
        event: &ProcessExit,
        ctx: &HandlerCtx,
    ) -> Result<(), PluginError> {
        let _ = (event, ctx);
        Ok(())
    }
}

#[async_trait]
pub trait ConsoleInterposer: Send + Sync {
    async fn on_console_input(&self, port: u32, bytes: &mut Vec<u8>) -> Result<(), PluginError> {
        let _ = (port, bytes);
        Ok(())
    }
    async fn on_console_output(&self, port: u32, chunk: &mut Vec<u8>) -> Result<(), PluginError> {
        let _ = (port, chunk);
        Ok(())
    }
    async fn on_exec_stdout(
        &self,
        pid: u32,
        exec_id: u64,
        chunk: &mut Vec<u8>,
    ) -> Result<(), PluginError> {
        let _ = (pid, exec_id, chunk);
        Ok(())
    }
    async fn on_exec_stderr(
        &self,
        pid: u32,
        exec_id: u64,
        chunk: &mut Vec<u8>,
    ) -> Result<(), PluginError> {
        let _ = (pid, exec_id, chunk);
        Ok(())
    }
}

#[async_trait]
pub trait SystemTelemetryParticipant: Send + Sync {
    async fn on_memory_pressure(
        &self,
        available_mb: u32,
        total_mb: u32,
    ) -> Result<(), PluginError> {
        let _ = (available_mb, total_mb);
        Ok(())
    }
    async fn on_heartbeat(&self, uptime_ms: u64) -> Result<(), PluginError> {
        let _ = uptime_ms;
        Ok(())
    }
    async fn on_watchdog(&self, event: &WatchdogEvent) -> Result<WatchdogVerdict, PluginError> {
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

// see: cans/11-protocol.md#Control-channel
// see: cans/plugins/tool-guard.md
#[async_trait]
pub trait StreamInterposer: Send + Sync {
    async fn on_stream_open(
        &self,
        stream_id: u32,
        tags: &mut Vec<u8>,
        ctx: &HandlerCtx,
    ) -> Result<Verdict, PluginError> {
        let _ = (stream_id, tags, ctx);
        Ok(Verdict::Pass)
    }
    async fn on_stream_data(
        &self,
        direction: StreamDirection,
        stream_id: u32,
        payload: &mut Vec<u8>,
        ctx: &HandlerCtx,
    ) -> Result<Verdict, PluginError> {
        let _ = (direction, stream_id, payload, ctx);
        Ok(Verdict::Pass)
    }
    async fn on_stream_close(&self, stream_id: u32, ctx: &HandlerCtx) -> Result<(), PluginError> {
        let _ = (stream_id, ctx);
        Ok(())
    }
}

pub trait ForeignPluginBridge: KelirPlugin {
    fn mount_scope(&self) -> &MountScope;
    fn max_dispatches_per_second(&self) -> u32 {
        10_000
    }
    fn worker_channel_capacity(&self) -> usize {
        4_096
    }
    fn buffer_ceiling(&self) -> usize {
        64 * 1024
    }
}
