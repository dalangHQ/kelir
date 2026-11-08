// see: cans/01-architecture.md#Host-plane-subsystem
// see: cans/01-architecture.md#Host-memory-pressure-watcher
// see: cans/08-lifecycle.md#Host-crash-reaper
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};

use kelir_interface::driver::BalloonConvergence;
use kelir_interface::handles::VmControl;
use kelir_interface::plugin::PluginError;

pub enum HostReaperKind {
    LinuxPdeathsig,
    DarwinKqueue,
    WindowsJobObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureSource {
    LinuxCgroupV2MemoryPressure,
    MacosDispatchMemoryPressure,
    WindowsJobObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureLevel {
    None,
    Low,
    Medium,
    High,
    Critical,
}

impl PressureLevel {
    pub fn from_u8(level: u8) -> Self {
        match level {
            0 => Self::None,
            1 => Self::Low,
            2 => Self::Medium,
            3 => Self::High,
            _ => Self::Critical,
        }
    }

    pub fn as_u8(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 3,
            Self::Critical => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PressureEvent {
    pub level: PressureLevel,
    pub at_ns: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PressureSubscription {
    pub source: PressureSource,
    pub armed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BalloonRequest {
    pub target_mb: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReclaimRequest {
    pub bytes: u64,
}

pub struct HostPlane {
    reaper_kind: HostReaperKind,
    pressure_level: AtomicU8,
    watcher: Mutex<Option<PressureSubscription>>,
    vm_control: Option<std::sync::Arc<dyn VmControl>>,
    metrics: std::sync::Arc<kelir_otlp::registry::MetricRegistry>,
}

impl Default for HostPlane {
    fn default() -> Self {
        Self::new()
    }
}

impl HostPlane {
    pub fn new() -> Self {
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
            vm_control: None,
            metrics: std::sync::Arc::new(kelir_otlp::registry::MetricRegistry::new()),
        }
    }

    // see: cans/08-lifecycle.md#Hypervisor-primitives
    pub fn with_vm_control(mut self, control: std::sync::Arc<dyn VmControl>) -> Self {
        self.vm_control = Some(control);
        self
    }

    pub fn with_metrics(
        mut self,
        metrics: std::sync::Arc<kelir_otlp::registry::MetricRegistry>,
    ) -> Self {
        self.metrics = metrics;
        self
    }

    pub fn metrics(&self) -> &std::sync::Arc<kelir_otlp::registry::MetricRegistry> {
        &self.metrics
    }

    // see: cans/14-observability.md#Metrics-pipeline
    pub fn refresh_rss(&self) -> Option<u64> {
        #[cfg(target_os = "linux")]
        {
            let status = std::fs::read_to_string("/proc/self/status").ok()?;
            for line in status.lines() {
                let Some(rest) = line.strip_prefix("VmRSS:") else {
                    continue;
                };
                let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
                let bytes = kb.saturating_mul(1024);
                self.metrics.set_rss_bytes(bytes);
                return Some(bytes);
            }
            None
        }
        #[cfg(not(target_os = "linux"))]
        {
            None
        }
    }

    pub fn lsm_status(&self) -> bool {
        // TODO: query the host LSM (Linux LSM / macOS sandbox / Windows integrity) status.
        let _ = &self.reaper_kind;
        false
    }

    pub fn pressure_level(&self) -> u8 {
        self.pressure_level.load(Ordering::SeqCst)
    }

    pub fn pressure_level_typed(&self) -> PressureLevel {
        PressureLevel::from_u8(self.pressure_level())
    }

    pub fn subscribe_pressure(
        &self,
    ) -> Result<PressureSubscription, kelir_interface::plugin::PluginError> {
        // TODO (L3): subscribe to the kernel pressure source (cgroup v2 memory.pressure via epoll/io_uring, DISPATCH_SOURCE_TYPE_MEMORYPRESSURE on macOS) and store the subscription in watcher.
        let _ = &self.watcher;
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO (L3): HostPlane::subscribe_pressure".into(),
        ))
    }

    pub fn poll_pressure_events(
        &self,
    ) -> Result<Vec<PressureEvent>, kelir_interface::plugin::PluginError> {
        // TODO (L3): drain kernel pressure events into PressureEvents and update pressure_level.
        let _ = &self.watcher;
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO (L3): HostPlane::poll_pressure_events".into(),
        ))
    }

    // see: cans/08-lifecycle.md#Hypervisor-primitives
    pub async fn inflate_balloon(
        &self,
        request: BalloonRequest,
    ) -> Result<BalloonConvergence, PluginError> {
        let Some(control) = &self.vm_control else {
            return Err(PluginError::Halt(
                "vm control not wired; inflate_balloon needs Session::boot to bind the driver control"
                    .into(),
            ));
        };
        control.adjust_balloon(request.target_mb).await
    }

    pub fn reclaim(
        &self,
        request: ReclaimRequest,
    ) -> Result<u64, kelir_interface::plugin::PluginError> {
        // TODO (L3): reclaim request.bytes of host frames via madvise(MADV_DONTNEED) on Linux, hv_vm_unmap + mach_vm_deallocate on macOS.
        let _ = request;
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO (L3): HostPlane::reclaim".into(),
        ))
    }

    pub fn arm_host_reaper(
        &self,
        parent_pid: u32,
    ) -> Result<(), kelir_interface::plugin::PluginError> {
        // TODO (L3): arm the platform reaper (pdeathsig / kqueue NOTE_EXIT / Job Object) on parent_pid.
        let _ = parent_pid;
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO (L3): HostPlane::arm_host_reaper".into(),
        ))
    }
}
