// see: cans/02-plugin-model.md#Static-dispatch
// see: cans/02-plugin-model.md#Unified-interposition-pipeline
#[path = "console_dispatch.flow.rs"]
mod console_dispatch;
#[path = "engine.flow.rs"]
mod engine;
#[path = "fs_dispatch.flow.rs"]
mod fs_dispatch;
#[path = "host_dispatch.flow.rs"]
mod host_dispatch;
#[path = "lifecycle_dispatch.flow.rs"]
mod lifecycle_dispatch;
mod params_hydration;
#[path = "plugin_registration.flow.rs"]
mod plugin_registration;
#[path = "quarantine.flow.rs"]
mod quarantine;
#[path = "registration.flow.rs"]
mod registration;

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

pub use engine::PipelineEngine;
pub(crate) use quarantine::{
    Quarantined, map_reject_to_filter_action, map_reject_to_verdict, quarantine_sync,
    quarantine_unit, quarantine_verdict,
};
pub use registration::{
    CustomActionBinding, CustomActionCallback, HostRegistrationState, TrapDenyCallback,
};
