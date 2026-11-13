// see: cans/01-architecture.md#Host-plane-subsystem
// see: cans/01-architecture.md#Host-memory-pressure-watcher
// see: cans/08-lifecycle.md#Host-crash-reaper
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use kelir_interface::driver_interface::BalloonConvergence;
use kelir_interface::handles_lifecycle::VmControl;
use kelir_interface::host::{
    BalloonRequest, HostOps, HostReaperKind, PressureEvent, PressureLevel, PressureSubscription,
    ReclaimRequest,
};
use kelir_interface::plugin_model::PluginError;
use kelir_otlp::registry_otlp::MetricRegistry;

pub struct RealHostOps {
    pub(crate) reaper_kind: HostReaperKind,
    pub(crate) pressure_level: AtomicU8,
    pub(crate) watcher: Mutex<Option<PressureSubscription>>,
    pub(crate) vm_control: Mutex<Option<Arc<dyn VmControl>>>,
    pub(crate) metrics: Arc<MetricRegistry>,
}

impl RealHostOps {
    pub fn new(metrics: Arc<MetricRegistry>) -> Self {
        let reaper_kind = {
            #[cfg(target_os = "linux")]
            {
                HostReaperKind::LinuxPdeathsig
            }
            #[cfg(target_os = "macos")]
            {
                HostReaperKind::DarwinKqueue
            }
            #[cfg(target_os = "windows")]
            {
                HostReaperKind::WindowsJobObject
            }
            #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
            {
                // see: cans/08-lifecycle.md#Host-crash-reaper
                HostReaperKind::LinuxPdeathsig
            }
        };
        Self {
            reaper_kind,
            pressure_level: AtomicU8::new(0),
            watcher: Mutex::new(None),
            vm_control: Mutex::new(None),
            metrics,
        }
    }
}

#[async_trait]
impl HostOps for RealHostOps {
    fn set_vm_control(&self, control: Arc<dyn VmControl>) {
        if let Ok(mut bound) = self.vm_control.lock() {
            *bound = Some(control);
        }
    }

    fn fresh(&self) -> Arc<dyn HostOps> {
        Arc::new(Self::new(Arc::clone(&self.metrics)))
    }

    // see: cans/14-observability.md#Metrics-pipeline
    fn refresh_rss(&self) -> Option<u64> {
        crate::reaper::refresh_rss(&self.metrics)
    }

    fn lsm_status(&self) -> bool {
        crate::reaper::lsm_status(&self.reaper_kind)
    }

    fn pressure_level(&self) -> u8 {
        self.pressure_level.load(Ordering::SeqCst)
    }

    fn pressure_level_typed(&self) -> PressureLevel {
        PressureLevel::from_u8(self.pressure_level())
    }

    fn subscribe_pressure(&self) -> Result<PressureSubscription, PluginError> {
        crate::pressure::subscribe_pressure(&self.watcher)
    }

    fn poll_pressure_events(&self) -> Result<Vec<PressureEvent>, PluginError> {
        crate::pressure::poll_pressure_events(&self.watcher)
    }

    // see: cans/08-lifecycle.md#Hypervisor-primitives
    async fn inflate_balloon(
        &self,
        request: BalloonRequest,
    ) -> Result<BalloonConvergence, PluginError> {
        crate::balloon::inflate_balloon(&self.vm_control, request).await
    }

    fn reclaim(&self, request: ReclaimRequest) -> Result<u64, PluginError> {
        crate::balloon::reclaim(request)
    }

    fn arm_host_reaper(&self, parent_pid: u32) -> Result<(), PluginError> {
        crate::reaper::arm_host_reaper(parent_pid)
    }
}
