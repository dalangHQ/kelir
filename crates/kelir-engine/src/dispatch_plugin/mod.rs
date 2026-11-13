// see: cans/02-plugin-model.md#Static-dispatch
// see: cans/02-plugin-model.md#Unified-interposition-pipeline
#[path = "console_dispatch.flow.rs"]
mod console_dispatch;
#[path = "fs_dispatch.flow.rs"]
mod fs_dispatch;
#[path = "host_dispatch.flow.rs"]
mod host_dispatch;
#[path = "lifecycle_dispatch.flow.rs"]
mod lifecycle_dispatch;
mod params_hydration;
#[path = "plugin_registration.flow.rs"]
mod plugin_registration;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};
use std::time::{SystemTime, UNIX_EPOCH};

use kelir_interface::checkpoint::PidLineage;
use kelir_interface::driver_interface::{WatchdogEvent, WatchdogVerdict};
use kelir_interface::errno_types::Errno;
use kelir_interface::fusion_plugin::{
    FusedTrampolineCallback, FusedTrampolineDescriptor, TrampolineRing,
};
use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{
    ConsoleInterposer, FfiDispatchQueue, FsInterposer, HandlerCtx, HostInterposer, LifecycleCtx,
    LifecycleEvent, LifecycleParticipant, LockBrokerCoordination, MountScope, NetInterposer,
    ParkedConnectionReset, PipelineBinder, PostExecGate, StreamFrameSink,
    SystemTelemetryParticipant, TokenRevocation, TrapInterposer, VirtioDevice,
};
use kelir_interface::protocol_wire::{
    EXEC_STREAM_STDERR, EXEC_STREAM_STDOUT, FilterAction, Heartbeat, PostExecAction, ProcessExit,
    StepVerdict, StreamKind, TrapEvent, Verdict,
};
use kelir_interface::telemetry::{ExecutionReceipt, ReceiptStore};
use kelir_interface::terminal::LogStream;
use kelir_interface::vsock_mesh::{VsockServiceHandler, VsockServiceSpec};

use crate::planes::checkpoint::CheckpointPlane;
use crate::session_lifecycle::Session;
use crate::trap_gate::{GateDecision, TrapGateEvaluator};
#[path = "engine.flow.rs"]
mod engine;
#[path = "quarantine.flow.rs"]
mod quarantine;

use quarantine::{
    Quarantined, map_reject_to_filter_action, map_reject_to_verdict, quarantine_sync,
    quarantine_unit, quarantine_verdict,
};

// see: cans/03-sdk-architecture.md#Builder-consumption-model-across-languages
pub type TrapDenyCallback = Box<
    dyn Fn(&TrapEvent, &Session) -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send>>
        + Send
        + Sync,
>;

// see: cans/03-sdk-architecture.md#Rust-SDK
pub type CustomActionCallback = Box<
    dyn Fn(
            ciborium::value::Value,
        ) -> Pin<
            Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send>,
        > + Send
        + Sync,
>;

// see: cans/03-sdk-architecture.md#Rust-SDK
pub struct CustomActionBinding {
    pub name: String,
    pub handler: CustomActionCallback,
}

// see: cans/08-lifecycle.md#CoW-fork
#[derive(Clone)]
pub struct HostRegistrationState {
    pub on_trap_deny: Option<Arc<TrapDenyCallback>>,
    pub custom_actions: Vec<Arc<CustomActionBinding>>,
    pub stream_kind_services: Vec<(StreamKind, Arc<dyn StreamFrameSink>)>,
    pub lock_broker: Option<Arc<dyn LockBrokerCoordination>>,
    pub token_revoker: Option<Arc<dyn TokenRevocation>>,
}
pub struct PipelineEngine {
    trap_interceptors: Vec<Arc<dyn TrapInterposer>>,
    trap_scopes: Vec<MountScope>,
    gate: TrapGateEvaluator,
    fs_interceptors: Vec<Arc<dyn FsInterposer>>,
    fs_scopes: Vec<MountScope>,
    net_interceptors: Vec<Arc<dyn NetInterposer>>,
    net_scopes: Vec<MountScope>,
    console_interceptors: Vec<Arc<dyn ConsoleInterposer>>,
    console_scopes: Vec<MountScope>,
    terminal: Option<Arc<crate::planes::terminal::TerminalPlane>>,
    host_interceptors: Vec<Arc<dyn HostInterposer>>,
    host_scopes: Vec<MountScope>,
    vsock_services: Vec<(VsockServiceSpec, Arc<dyn VsockServiceHandler>, MountScope)>,
    lifecycle_interceptors: Vec<Arc<dyn LifecycleParticipant>>,
    lifecycle_scopes: Vec<MountScope>,
    telemetry_interceptors: Vec<Arc<dyn SystemTelemetryParticipant>>,
    telemetry_scopes: Vec<MountScope>,
    post_exec_gates: Vec<Arc<dyn PostExecGate>>,
    post_exec_scopes: Vec<MountScope>,
    stream_sinks: Vec<(StreamKind, Arc<dyn StreamFrameSink>, MountScope)>,
    lock_broker: Option<Arc<dyn LockBrokerCoordination>>,
    parked_reset: Option<Arc<dyn ParkedConnectionReset>>,
    token_revoker: Option<Arc<dyn TokenRevocation>>,
    metrics: Arc<kelir_otlp::registry_otlp::MetricRegistry>,
    trampoline: Option<kelir_interface::fusion_plugin::FusedTrampolineDescriptor>,
    checkpoint: Option<Arc<CheckpointPlane>>,
    on_trap_deny: Option<Arc<TrapDenyCallback>>,
    custom_actions: Vec<Arc<CustomActionBinding>>,
    receipt_store: ReceiptStore,
    fused_callbacks: Vec<(TrampolineRing, u32, Arc<dyn FusedTrampolineCallback>)>,
    foreign_queue: FfiDispatchQueue,
    ffi_rate_per_sec: u32,
    ffi_window_start_ms: AtomicU64,
    ffi_window_ops: AtomicU64,
    active_scope: Option<MountScope>,
    sealed: bool,
}

impl Default for PipelineEngine {
    fn default() -> Self {
        Self::new()
    }
}
