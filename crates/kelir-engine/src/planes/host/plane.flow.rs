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

impl HostPlane {
    pub fn new(ops: Arc<dyn HostOps>) -> Self {
        Self {
            ops,
            metrics: Arc::new(MetricRegistry::new()),
        }
    }

    // see: cans/08-lifecycle.md#Hypervisor-primitives
    pub fn with_vm_control(self, control: Arc<dyn VmControl>) -> Self {
        self.ops.set_vm_control(control);
        self
    }

    pub fn with_metrics(mut self, metrics: Arc<MetricRegistry>) -> Self {
        self.metrics = metrics;
        self
    }

    pub fn metrics(&self) -> &Arc<MetricRegistry> {
        &self.metrics
    }

    // see: cans/14-observability.md#Metrics-pipeline
    pub fn refresh_rss(&self) -> Option<u64> {
        self.ops.refresh_rss()
    }

    pub fn lsm_status(&self) -> bool {
        self.ops.lsm_status()
    }

    pub fn pressure_level(&self) -> u8 {
        self.ops.pressure_level()
    }

    pub fn pressure_level_typed(&self) -> PressureLevel {
        self.ops.pressure_level_typed()
    }

    pub fn subscribe_pressure(&self) -> Result<PressureSubscription, PluginError> {
        self.ops.subscribe_pressure()
    }

    pub fn poll_pressure_events(&self) -> Result<Vec<PressureEvent>, PluginError> {
        self.ops.poll_pressure_events()
    }

    // see: cans/08-lifecycle.md#Hypervisor-primitives
    pub async fn inflate_balloon(
        &self,
        request: BalloonRequest,
    ) -> Result<BalloonConvergence, PluginError> {
        self.ops.inflate_balloon(request).await
    }

    pub fn reclaim(&self, request: ReclaimRequest) -> Result<u64, PluginError> {
        self.ops.reclaim(request)
    }

    pub fn arm_host_reaper(&self, parent_pid: u32) -> Result<(), PluginError> {
        self.ops.arm_host_reaper(parent_pid)
    }
}
