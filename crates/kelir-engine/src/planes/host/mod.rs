// see: cans/01-architecture.md#Host-plane-subsystem
// see: cans/01-architecture.md#Host-memory-pressure-watcher
// see: cans/08-lifecycle.md#Host-crash-reaper
use std::sync::Arc;

use kelir_interface::driver_interface::BalloonConvergence;
use kelir_interface::handles_lifecycle::VmControl;
use kelir_interface::host::{
    BalloonRequest, HostOps, PressureEvent, PressureLevel, PressureSubscription, ReclaimRequest,
};
use kelir_interface::plugin_model::PluginError;
use kelir_otlp::registry_otlp::MetricRegistry;

pub struct HostPlane {
    ops: Arc<dyn HostOps>,
    metrics: Arc<MetricRegistry>,
}
#[path = "plane.flow.rs"]
mod plane;
