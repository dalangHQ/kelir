// see: cans/01-architecture.md#Host-plane-subsystem
// see: cans/01-architecture.md#Host-memory-pressure-watcher
// see: cans/08-lifecycle.md#Host-crash-reaper
use std::sync::atomic::AtomicU8;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use kelir_interface::driver_interface::BalloonConvergence;
use kelir_interface::handles_lifecycle::VmControl;
use kelir_interface::host::{
    BalloonRequest, HostOps, HostReaperKind, PressureEvent, PressureLevel, PressureSubscription,
    ReclaimRequest,
};
use kelir_interface::plugin_model::PluginError;
use kelir_interface::telemetry::MetricRegistry;

// TODO (L3) [hypervisor_driver]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
pub(crate) struct RealHostOps {
    pub(crate) reaper_kind: HostReaperKind,
    pub(crate) pressure_level: AtomicU8,
    pub(crate) watcher: Mutex<Option<PressureSubscription>>,
    pub(crate) vm_control: Mutex<Option<Arc<dyn VmControl>>>,
    pub(crate) metrics: Arc<MetricRegistry>,
}

impl RealHostOps {
    pub(crate) fn new(metrics: Arc<MetricRegistry>) -> Self {
        // TODO (L3) [hypervisor_driver]: select HostReaperKind per target_os (LinuxPdeathsig/DarwinKqueue/WindowsJobObject).; slop: reaper kind never selected, the orphan hypervisor survives parent death on every OS.
        Self {
            reaper_kind: HostReaperKind::LinuxPdeathsig,
            pressure_level: AtomicU8::new(0),
            watcher: Mutex::new(None),
            vm_control: Mutex::new(None),
            metrics,
        }
    }
}

pub(crate) fn build_host_ops(metrics: Arc<MetricRegistry>) -> Arc<dyn HostOps> {
    Arc::new(RealHostOps::new(metrics))
}

#[async_trait]
impl HostOps for RealHostOps {
    fn set_vm_control(&self, _control: Arc<dyn VmControl>) {
        // TODO (L3) [hypervisor_driver]: bind the VmControl handle for balloon operations.; slop: balloon handle stays unbound, inflate and reclaim calls fail while reporting success.
    }

    fn fresh(&self) -> Arc<dyn HostOps> {
        Arc::new(Self::new(Arc::clone(&self.metrics)))
    }

    // see: cans/14-observability.md#Metrics-pipeline
    fn refresh_rss(&self) -> Option<u64> {
        // TODO (L3) [hypervisor_driver]: read host RSS and record it in metrics.; slop: host RSS never recorded, memory accounting reports zero while the host grows.
        None
    }

    fn lsm_status(&self) -> bool {
        // TODO (L3) [hypervisor_driver]: query the host LSM status for the reaper kind.; slop: LSM status never queried, the reaper arms against the wrong LSM assumptions.
        false
    }

    fn pressure_level(&self) -> u8 {
        // TODO (L3) [hypervisor_driver]: return the current memory pressure level as a u8.; slop: pressure always reads zero, the balloon never inflates under real pressure.
        0
    }

    fn pressure_level_typed(&self) -> PressureLevel {
        // TODO (L3) [hypervisor_driver]: return the current memory pressure level as a PressureLevel.; slop: pressure level never computed, subscribers act on a stale level.
        PressureLevel::None
    }

    fn subscribe_pressure(&self) -> Result<PressureSubscription, PluginError> {
        // TODO (L3) [hypervisor_driver]: subscribe to the kernel memory-pressure source and store the subscription.; slop: no kernel subscription, pressure events never arrive and reclaim never triggers.
        Err(PluginError::Halt(
            "pressure subscription pending (L3)".into(),
        ))
    }

    fn poll_pressure_events(&self) -> Result<Vec<PressureEvent>, PluginError> {
        // TODO (L3) [hypervisor_driver]: drain kernel pressure events into PressureEvents and update the pressure level.; slop: pressure events never drained, the level freezes while the kernel signals distress.
        Err(PluginError::Halt("pressure events pending (L3)".into()))
    }

    // see: cans/08-lifecycle.md#Hypervisor-primitives
    async fn inflate_balloon(
        &self,
        _request: BalloonRequest,
    ) -> Result<BalloonConvergence, PluginError> {
        // TODO (L3) [hypervisor_driver]: inflate the balloon to the requested target via the bound VmControl.; slop: balloon never inflates, the guest keeps memory the host needs back.
        Err(PluginError::Halt("balloon inflation pending (L3)".into()))
    }

    fn reclaim(&self, _request: ReclaimRequest) -> Result<u64, PluginError> {
        // TODO (L3) [hypervisor_driver]: reclaim the requested host frames via madvise/hv_vm_unmap and return bytes reclaimed.; slop: reclaim reports bytes without unmapping, host frames stay pinned.
        Err(PluginError::Halt("balloon reclaim pending (L3)".into()))
    }

    fn arm_host_reaper(&self, _parent_pid: u32) -> Result<(), PluginError> {
        // TODO (L3) [hypervisor_driver]: arm the platform reaper (pdeathsig/kqueue NOTE_EXIT/Job Object) on the parent pid.; slop: reaper never armed, the orphan hypervisor survives parent SIGKILL.
        Err(PluginError::Halt("host reaper arming pending (L3)".into()))
    }
}
