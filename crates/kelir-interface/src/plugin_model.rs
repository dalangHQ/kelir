// see: cans/02-plugin-model.md#KelirPlugin-trait
// see: cans/02-plugin-model.md#Segregated-ring-traits
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::driver_interface::{
    BalloonConvergence, EntropyBackend, EntropyStreamPosition, PmuSampler, VmConfig, VmmDriver,
    VmmError, WatchdogAction, WatchdogEvent, WatchdogVerdict,
};
use crate::errno_types::Errno;
pub use crate::handlers_plugin::{
    ConsoleContext, ExecContext, FsContext, HandlerCtx, Has, HostContext, NetContext,
};
use crate::protocol_wire::{
    FilterAction, Heartbeat, PostExecAction, ProcessExit, StreamKind, SuspendUntil, TrapEvent,
    Verdict,
};
use crate::vsock_mesh::{VsockServiceHandler, VsockServiceSpec};

// see: cans/02-plugin-model.md#Plugin-ownership
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginRegistration {
    pub name: String,
    pub enabled: bool,
}

impl PluginRegistration {
    pub fn new(name: impl Into<String>, enabled: bool) -> Self {
        Self {
            name: name.into(),
            enabled,
        }
    }
}

// see: cans/artifacts/code/plugin-trait.rs#PluginError
#[derive(Error, Debug)]
pub enum PluginError {
    #[error("Foreign FFI channel saturated: worker queue depth exceeded ({depth})")]
    FfiQueueSaturated { depth: usize },
    #[error("W^X execution violation detected: PID {pid} attempted {operation}")]
    WxViolation { pid: u32, operation: String },
    #[error("Ambiguous mutation detected across plugins on resource '{resource}': {conflict}")]
    AmbiguousMutation { resource: String, conflict: String },
    #[error("{errno}: {reason}")]
    Reject { errno: Errno, reason: String },
    #[error("Internal I/O failure: {0}")]
    Io(#[from] std::io::Error),
    #[error("Plugin halted: {0}")]
    Halt(String),
}

// see: cans/02-plugin-model.md#Declarative-error-mapping
impl PartialEq for PluginError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::FfiQueueSaturated { depth: left }, Self::FfiQueueSaturated { depth: right }) => {
                left == right
            }
            (
                Self::WxViolation {
                    pid: left_pid,
                    operation: left_operation,
                },
                Self::WxViolation {
                    pid: right_pid,
                    operation: right_operation,
                },
            ) => left_pid == right_pid && left_operation == right_operation,
            (
                Self::AmbiguousMutation {
                    resource: left_resource,
                    conflict: left_conflict,
                },
                Self::AmbiguousMutation {
                    resource: right_resource,
                    conflict: right_conflict,
                },
            ) => left_resource == right_resource && left_conflict == right_conflict,
            (
                Self::Reject {
                    errno: left_errno,
                    reason: left_reason,
                },
                Self::Reject {
                    errno: right_errno,
                    reason: right_reason,
                },
            ) => left_errno == right_errno && left_reason == right_reason,
            (Self::Io(left), Self::Io(right)) => left.kind() == right.kind(),
            (Self::Halt(left), Self::Halt(right)) => left == right,
            _ => false,
        }
    }
}

// see: cans/02-plugin-model.md#Declarative-error-mapping
impl Eq for PluginError {}

impl PluginError {
    pub fn ambiguous_mutation(resource: impl Into<String>, conflict: impl Into<String>) -> Self {
        Self::AmbiguousMutation {
            resource: resource.into(),
            conflict: conflict.into(),
        }
    }

    pub fn reject(errno: Errno, reason: impl Into<String>) -> Self {
        Self::Reject {
            errno,
            reason: reason.into(),
        }
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

// see: cans/artifacts/code/plugin-trait.rs#MountScope
// see: cans/02-plugin-model.md#Mount-scoping
#[derive(Debug, Clone, Default)]
pub struct MountScope {
    pub devices: Option<Vec<VirtioDevice>>,
    pub binaries: Option<Vec<String>>,
    pub paths: Option<Vec<PathBuf>>,
    pub stream_kinds: Option<Vec<StreamKind>>,
    // see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
    pub net_cidrs: Option<Vec<String>>,
    pub net_ports: Option<Vec<u16>>,
}

impl MountScope {
    pub fn with_devices<I: IntoIterator<Item = VirtioDevice>>(mut self, devices: I) -> Self {
        self.devices = Some(devices.into_iter().collect());
        self
    }

    pub fn with_binaries<S: Into<String>>(mut self, binaries: impl IntoIterator<Item = S>) -> Self {
        self.binaries = Some(binaries.into_iter().map(Into::into).collect());
        self
    }

    pub fn with_paths<I: IntoIterator<Item = P>, P: Into<PathBuf>>(mut self, paths: I) -> Self {
        self.paths = Some(paths.into_iter().map(Into::into).collect());
        self
    }

    // see: cans/02-plugin-model.md#Mount-scoping
    pub fn with_stream_kinds<I: IntoIterator<Item = StreamKind>>(mut self, kinds: I) -> Self {
        self.stream_kinds = Some(kinds.into_iter().collect());
        self
    }

    // see: cans/02-plugin-model.md#Mount-scoping
    pub fn with_net_cidrs<S: Into<String>>(mut self, cidrs: impl IntoIterator<Item = S>) -> Self {
        self.net_cidrs = Some(cidrs.into_iter().map(Into::into).collect());
        self
    }

    // see: cans/02-plugin-model.md#Mount-scoping
    pub fn with_net_ports<I: IntoIterator<Item = u16>>(mut self, ports: I) -> Self {
        self.net_ports = Some(ports.into_iter().collect());
        self
    }

    // see: cans/02-plugin-model.md#Mount-scoping
    fn intersect_list<T: PartialEq + Clone>(
        child: &Option<Vec<T>>,
        parent: &Option<Vec<T>>,
    ) -> Option<Vec<T>> {
        match (child, parent) {
            (Some(a), Some(b)) => Some(a.iter().filter(|x| b.contains(x)).cloned().collect()),
            (Some(a), None) => Some(a.clone()),
            (None, Some(b)) => Some(b.clone()),
            (None, None) => None,
        }
    }

    pub fn intersect(&self, parent: &Self) -> Self {
        Self {
            devices: Self::intersect_list(&self.devices, &parent.devices),
            binaries: Self::intersect_list(&self.binaries, &parent.binaries),
            paths: Self::intersect_list(&self.paths, &parent.paths),
            stream_kinds: Self::intersect_list(&self.stream_kinds, &parent.stream_kinds),
            net_cidrs: Self::intersect_list(&self.net_cidrs, &parent.net_cidrs),
            net_ports: Self::intersect_list(&self.net_ports, &parent.net_ports),
        }
    }

    // TODO (L3): canonical (st_dev, st_ino, binary_blake3) — cans/11-protocol.md#Trap-fast-path
    // see: cans/02-plugin-model.md#Filter-before-FFI-gate
    pub fn allows_binary(&self, _binary: &str) -> bool {
        self.binaries.is_none()
    }

    pub fn allows_path(&self, path: &Path) -> bool {
        match &self.paths {
            None => true,
            Some(list) => list.iter().any(|item| path.starts_with(item)),
        }
    }

    pub fn allows_device(&self, device: VirtioDevice) -> bool {
        match &self.devices {
            None => true,
            Some(list) => list.contains(&device),
        }
    }

    // see: cans/02-plugin-model.md#Mount-scoping
    pub fn allows_stream_kind(&self, kind: StreamKind) -> bool {
        match &self.stream_kinds {
            None => true,
            Some(list) => list.contains(&kind),
        }
    }

    // see: cans/02-plugin-model.md#Mount-scoping
    pub fn allows_net(&self, ip: [u8; 4], port: u16) -> bool {
        let cidr_ok = match &self.net_cidrs {
            None => true,
            Some(list) => list.is_empty() || list.iter().any(|cidr| cidr_contains(cidr, ip)),
        };
        let port_ok = match &self.net_ports {
            None => true,
            Some(list) => list.is_empty() || list.contains(&port),
        };
        cidr_ok && port_ok
    }
}

// see: cans/02-plugin-model.md#Mount-scoping
fn cidr_contains(cidr: &str, ip: [u8; 4]) -> bool {
    let Some((addr, prefix)) = cidr.split_once('/') else {
        return false;
    };
    let (Ok(prefix), Some(network)) = (prefix.parse::<u32>(), parse_ipv4(addr)) else {
        return false;
    };
    if prefix > 32 {
        return false;
    }
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    (u32::from_be_bytes(ip) & mask) == (network & mask)
}

// see: cans/02-plugin-model.md#Mount-scoping
fn parse_ipv4(addr: &str) -> Option<u32> {
    let mut parts = addr.split('.');
    let mut octets = [0u8; 4];
    for slot in &mut octets {
        *slot = parts.next()?.parse().ok()?;
    }
    parts.next().is_none().then_some(u32::from_be_bytes(octets))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

// see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
#[async_trait]
pub trait LockBrokerCoordination: Send + Sync {
    async fn release_transaction_locks(&self) -> Result<(), PluginError>;
    // see: cans/05-storage-plane.md#Host-inode-lock-broker
    async fn release_transaction_locks_for(&self, namespace: [u8; 16]) -> Result<(), PluginError>;
}

// see: cans/plugins/wire-ingress.md#Connection-parking-on-suspend
pub trait ParkedConnectionReset: Send + Sync {
    fn reset_parked_connections(&self) -> Result<Vec<u64>, PluginError>;
}

// see: cans/08-lifecycle.md#Rollback
#[async_trait]
pub trait TokenRevocation: Send + Sync {
    async fn revoke_session_tokens(&self, session_id: &str) -> Result<(), PluginError>;
}

// see: cans/artifacts/code/plugin-trait.rs#LifecycleCtx
pub type VetoList = Vec<(VetoEvent, Box<dyn Fn() -> bool + Send + Sync>)>;

// see: cans/artifacts/code/plugin-trait.rs#LifecycleCtx
fn vmm_error(error: VmmError) -> PluginError {
    PluginError::Io(std::io::Error::other(error))
}

// see: cans/artifacts/code/plugin-trait.rs#LifecycleCtx
// TODO (L3): veto callback boxes carry Send + Sync beyond the artifact's Box<dyn Fn() -> bool> so LifecycleCtx stays Send across async await points.
pub struct LifecycleCtx<'a> {
    pub config: &'a VmConfig,
    pub session_id: &'a str,
    driver: &'a mut dyn VmmDriver,
    vetoes: &'a mut VetoList,
    watchdog_probe: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl<'a> LifecycleCtx<'a> {
    pub fn new(
        config: &'a VmConfig,
        session_id: &'a str,
        driver: &'a mut dyn VmmDriver,
        vetoes: &'a mut VetoList,
    ) -> Self {
        Self {
            config,
            session_id,
            driver,
            vetoes,
            watchdog_probe: None,
        }
    }

    // see: cans/08-lifecycle.md#Watchdog
    pub fn new_with_watchdog_probe(
        config: &'a VmConfig,
        session_id: &'a str,
        driver: &'a mut dyn VmmDriver,
        vetoes: &'a mut VetoList,
        watchdog_probe: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        Self {
            watchdog_probe: Some(watchdog_probe),
            ..Self::new(config, session_id, driver, vetoes)
        }
    }

    // see: cans/artifacts/code/plugin-trait.rs#LifecycleCtx
    pub async fn set_watchdog(
        &mut self,
        timeout: Duration,
        action: WatchdogAction,
    ) -> Result<(), PluginError> {
        if let Some(probe) = &self.watchdog_probe {
            probe.store(true, std::sync::atomic::Ordering::Release);
        }
        self.driver
            .set_watchdog(timeout, action)
            .await
            .map_err(vmm_error)
    }

    // see: cans/artifacts/code/plugin-trait.rs#LifecycleCtx
    pub async fn adjust_balloon(
        &mut self,
        target_mb: u32,
    ) -> Result<BalloonConvergence, PluginError> {
        self.driver
            .adjust_balloon(target_mb)
            .await
            .map_err(vmm_error)
    }

    // see: cans/artifacts/code/plugin-trait.rs#LifecycleCtx
    pub fn register_veto(
        &mut self,
        event: VetoEvent,
        callback: Box<dyn Fn() -> bool + Send + Sync>,
    ) {
        self.vetoes.push((event, callback));
    }

    // see: cans/10-attestation.md#Lifecycle-bridge
    pub async fn freeze_vcpus(&mut self) -> Result<(), PluginError> {
        self.driver.freeze_vcpus().await.map_err(vmm_error)
    }

    // see: cans/10-attestation.md#Lifecycle-bridge
    pub async fn unfreeze_vcpus(&mut self) -> Result<(), PluginError> {
        self.driver.unfreeze_vcpus().await.map_err(vmm_error)
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    pub async fn drain_fuse(&mut self, timeout: Duration) -> Result<(), PluginError> {
        self.driver.drain_fuse(timeout).await.map_err(vmm_error)
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    pub async fn commit_broker_upper(&mut self) -> Result<(), PluginError> {
        self.driver.commit_broker_upper().await.map_err(vmm_error)
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    pub async fn discard_broker_upper(&mut self) -> Result<(), PluginError> {
        self.driver.discard_broker_upper().await.map_err(vmm_error)
    }

    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    pub async fn step_clock_window(
        &mut self,
        retired_instructions: u64,
    ) -> Result<(), PluginError> {
        self.driver
            .step_clock_window(retired_instructions)
            .await
            .map_err(vmm_error)
    }

    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    pub fn register_pmu_sampler(
        &mut self,
        sampler: Arc<dyn PmuSampler>,
    ) -> Result<(), PluginError> {
        self.driver.register_pmu_sampler(sampler).map_err(vmm_error)
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    pub async fn set_entropy_backend(
        &mut self,
        backend: EntropyBackend,
    ) -> Result<(), PluginError> {
        self.driver
            .set_entropy_backend(backend)
            .await
            .map_err(vmm_error)
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    pub async fn entropy_position(&self) -> Result<EntropyStreamPosition, PluginError> {
        self.driver.entropy_position().await.map_err(vmm_error)
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    pub async fn restore_entropy_position(
        &mut self,
        position: EntropyStreamPosition,
    ) -> Result<(), PluginError> {
        self.driver
            .restore_entropy_position(position)
            .await
            .map_err(vmm_error)
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
    fn register_interceptors(&self, _pipeline: &mut dyn PipelineBinder) {}
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
    fn is_enabled(&self) -> bool {
        true
    }
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

    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        KelirPlugin::register_interceptors(self, pipeline)
    }

    fn default_scope(&self) -> MountScope {
        KelirPlugin::default_scope(self)
    }

    fn is_enabled(&self) -> bool {
        KelirPlugin::is_enabled(self)
    }
}

// see: cans/11-protocol.md#Control-channel
// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
// see: cans/artifacts/code/plugin-trait.rs#PipelineBinder
pub trait PipelineBinder {
    // see: cans/02-plugin-model.md#Mount-scoping
    fn bind_fs(&mut self, stage: Arc<dyn FsInterposer>) {
        self.bind_fs_scoped(stage, MountScope::default());
    }
    fn bind_fs_scoped(&mut self, stage: Arc<dyn FsInterposer>, scope: MountScope);
    fn bind_net(&mut self, stage: Arc<dyn NetInterposer>) {
        self.bind_net_scoped(stage, MountScope::default());
    }
    fn bind_net_scoped(&mut self, stage: Arc<dyn NetInterposer>, scope: MountScope);
    fn bind_trap(&mut self, stage: Arc<dyn TrapInterposer>) {
        self.bind_trap_scoped(stage, MountScope::default());
    }
    fn bind_trap_scoped(&mut self, stage: Arc<dyn TrapInterposer>, scope: MountScope);
    // see: cans/10-attestation.md#Step-evaluation-loop
    fn bind_post_exec(&mut self, gate: Arc<dyn PostExecGate>) {
        self.bind_post_exec_scoped(gate, MountScope::default());
    }
    fn bind_post_exec_scoped(&mut self, gate: Arc<dyn PostExecGate>, scope: MountScope);
    fn bind_console(&mut self, stage: Arc<dyn ConsoleInterposer>) {
        self.bind_console_scoped(stage, MountScope::default());
    }
    fn bind_console_scoped(&mut self, stage: Arc<dyn ConsoleInterposer>, scope: MountScope);
    fn bind_host(&mut self, stage: Arc<dyn HostInterposer>) {
        self.bind_host_scoped(stage, MountScope::default());
    }
    fn bind_host_scoped(&mut self, stage: Arc<dyn HostInterposer>, scope: MountScope);
    fn bind_vsock_service(
        &mut self,
        spec: VsockServiceSpec,
        service: Arc<dyn VsockServiceHandler>,
    ) {
        self.bind_vsock_service_scoped(spec, service, MountScope::default());
    }
    fn bind_vsock_service_scoped(
        &mut self,
        spec: VsockServiceSpec,
        service: Arc<dyn VsockServiceHandler>,
        scope: MountScope,
    );
    // see: cans/02-plugin-model.md#Segregated-ring-traits
    fn bind_lifecycle(&mut self, stage: Arc<dyn LifecycleParticipant>) {
        self.bind_lifecycle_scoped(stage, MountScope::default());
    }
    fn bind_lifecycle_scoped(&mut self, stage: Arc<dyn LifecycleParticipant>, scope: MountScope);
    // see: cans/02-plugin-model.md#Segregated-ring-traits
    fn bind_telemetry(&mut self, stage: Arc<dyn SystemTelemetryParticipant>) {
        self.bind_telemetry_scoped(stage, MountScope::default());
    }
    fn bind_telemetry_scoped(
        &mut self,
        stage: Arc<dyn SystemTelemetryParticipant>,
        scope: MountScope,
    );
    // see: cans/11-protocol.md#Kind-registration
    fn bind_stream_kind(&mut self, kind: StreamKind, sink: Arc<dyn StreamFrameSink>) {
        self.bind_stream_kind_scoped(kind, sink, MountScope::default());
    }
    fn bind_stream_kind_scoped(
        &mut self,
        kind: StreamKind,
        sink: Arc<dyn StreamFrameSink>,
        scope: MountScope,
    );
    // see: cans/plugins/wire-ingress.md#Connection-parking-on-suspend
    fn bind_parked_connection_reset(&mut self, reset: Arc<dyn ParkedConnectionReset>);
    // see: cans/08-lifecycle.md#Rollback
    fn bind_token_revoker(&mut self, revoker: Arc<dyn TokenRevocation>);
    // see: cans/02-plugin-model.md#Mount-scoping
    fn set_active_scope(&mut self, _scope: MountScope) {}
}

pub trait StreamFrameSink: Send + Sync {
    fn on_stream_frame(
        &self,
        stream_id: u32,
        kind: StreamKind,
        flags: u8,
        payload: &[u8],
    ) -> Result<(), PluginError>;
}

// see: cans/artifacts/code/plugin-trait.rs#EventBusBinder
pub type ErasedEventHandler = Box<dyn Fn(&dyn Any) + Send + Sync>;

// see: cans/artifacts/code/plugin-trait.rs#EventBusBinder
pub trait EventBusBinder {
    fn subscribe_erased(&mut self, event: TypeId, handler: ErasedEventHandler);
}

// see: cans/artifacts/code/plugin-trait.rs#EventBusSubscribe
pub trait EventBusSubscribe: EventBusBinder {
    fn subscribe<E: Any + Send + Sync + 'static>(&mut self, handler: Box<dyn Fn(&E) + Send + Sync>);
}

impl<B: EventBusBinder + ?Sized> EventBusSubscribe for B {
    fn subscribe<E: Any + Send + Sync + 'static>(
        &mut self,
        handler: Box<dyn Fn(&E) + Send + Sync>,
    ) {
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

// see: cans/02-plugin-model.md#Reactive-event-bus-plane
#[derive(Default)]
pub struct EventBus {
    handlers: HashMap<TypeId, Vec<ErasedEventHandler>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    // see: cans/02-plugin-model.md#Reactive-event-bus-plane
    pub fn publish<E: Any + Send + Sync>(&self, event: &E) {
        let Some(handlers) = self.handlers.get(&TypeId::of::<E>()) else {
            return;
        };
        for handler in handlers {
            handler(event as &dyn Any);
        }
    }
}

impl EventBusBinder for EventBus {
    fn subscribe_erased(&mut self, event: TypeId, handler: ErasedEventHandler) {
        self.handlers.entry(event).or_default().push(handler);
    }
}

pub trait CompositePlugin: Send + Sync {
    fn unpack(self: Box<Self>, parent_scope: &MountScope) -> Vec<(Box<dyn AnyPlugin>, MountScope)>;
}

// see: cans/artifacts/code/plugin-trait.rs#KelirPlugin
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
    fn register_subscribers(&self, _bus: &mut dyn EventBusBinder) {}
}

// see: cans/02-plugin-model.md#KelirPlugin-trait
pub type NoDeps = HNil;

// see: cans/02-plugin-model.md#Compile-time-prerequisites
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HNil;

// see: cans/02-plugin-model.md#Compile-time-prerequisites
#[derive(Clone, Debug)]
pub struct HCons<Head, Tail> {
    pub head: Head,
    pub tail: Tail,
}

// see: cans/02-plugin-model.md#KelirPlugin-trait
pub trait PluginDeps {
    type Deps;
}

#[async_trait]
pub trait LifecycleParticipant: Send + Sync {
    async fn on_vm_init(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        let _ = ctx;
        Ok(())
    }
    async fn on_vm_ready(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        let _ = ctx;
        Ok(())
    }
    async fn on_vm_shutdown(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        let _ = ctx;
        Ok(())
    }
    async fn on_pre_commit(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        let _ = ctx;
        Ok(())
    }
    async fn on_post_commit(
        &self,
        ctx: &mut LifecycleCtx<'_>,
        committed: bool,
    ) -> Result<(), PluginError> {
        let _ = (ctx, committed);
        Ok(())
    }
    async fn on_host_preempt(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        let _ = ctx;
        Ok(())
    }
}

// see: cans/artifacts/code/plugin-trait.rs#TrapInterposer
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

// see: cans/10-attestation.md#Step-evaluation-loop
#[async_trait]
pub trait PostExecGate: Send + Sync {
    async fn check(
        &self,
        event: &TrapEvent,
        ctx: &HandlerCtx,
    ) -> Result<PostExecAction, PluginError> {
        let _ = (event, ctx);
        Ok(PostExecAction::Proceed)
    }
}

#[async_trait]
pub trait ConsoleInterposer: Send + Sync {
    async fn on_console_input(&self, port: u32, bytes: &[u8]) -> Result<Vec<u8>, PluginError> {
        let _ = port;
        Ok(bytes.to_vec())
    }
    async fn on_console_output(&self, port: u32, chunk: &[u8]) -> Result<Vec<u8>, PluginError> {
        let _ = port;
        Ok(chunk.to_vec())
    }
    async fn on_exec_stdout(
        &self,
        pid: u32,
        exec_id: u64,
        chunk: &[u8],
    ) -> Result<Vec<u8>, PluginError> {
        let _ = (pid, exec_id);
        Ok(chunk.to_vec())
    }
    async fn on_exec_stderr(
        &self,
        pid: u32,
        exec_id: u64,
        chunk: &[u8],
    ) -> Result<Vec<u8>, PluginError> {
        let _ = (pid, exec_id);
        Ok(chunk.to_vec())
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
    async fn on_heartbeat(&self, event: &Heartbeat) -> Result<(), PluginError> {
        let _ = event;
        Ok(())
    }
    async fn on_watchdog(&self, event: &WatchdogEvent) -> Result<WatchdogVerdict, PluginError> {
        let _ = event;
        Ok(WatchdogVerdict::ExecuteDefault)
    }
}

// see: cans/artifacts/code/plugin-trait.rs#FsInterposer
// see: cans/02-plugin-model.md#Shape-of-the-surface
pub trait FsInterposer: Send + Sync {
    fn on_fs_lookup(&self, path: &Path) -> Result<Verdict, PluginError> {
        let _ = path;
        Ok(Verdict::Pass)
    }
    fn on_fs_read(&self, path: &Path, buffer: &[u8]) -> Result<Vec<u8>, PluginError> {
        let _ = path;
        Ok(buffer.to_vec())
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

// see: cans/artifacts/code/plugin-trait.rs#NetInterposer
// see: cans/02-plugin-model.md#Shape-of-the-surface
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

// see: cans/artifacts/code/plugin-trait.rs#HostInterposer
// see: cans/02-plugin-model.md#Shape-of-the-surface
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

// see: cans/plugins/tool-guard.md#Total-tool-call-interposition
// see: cans/11-protocol.md#Kind-registration

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

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
pub const FFI_DISPATCH_QUEUE_CAPACITY: usize = 4_096;

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
pub struct FfiDispatchQueue {
    capacity: usize,
    depth: AtomicUsize,
}

impl FfiDispatchQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            depth: AtomicUsize::new(0),
        }
    }

    pub fn try_submit(&self) -> Result<(), PluginError> {
        let mut current = self.depth.load(Ordering::Acquire);
        loop {
            if current >= self.capacity {
                return Err(PluginError::FfiQueueSaturated { depth: current });
            }
            match self.depth.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(()),
                Err(actual) => current = actual,
            }
        }
    }

    pub fn complete(&self) {
        let mut current = self.depth.load(Ordering::Acquire);
        while current > 0 {
            match self.depth.compare_exchange_weak(
                current,
                current - 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    pub fn depth(&self) -> usize {
        self.depth.load(Ordering::Acquire)
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

impl Default for FfiDispatchQueue {
    fn default() -> Self {
        Self::new(FFI_DISPATCH_QUEUE_CAPACITY)
    }
}

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
pub fn foreign_suspend_action() -> FilterAction {
    FilterAction::Suspend {
        until: SuspendUntil::Approval,
    }
}
