// see: cans/01-architecture.md#Host-plane-subsystem
// see: cans/08-lifecycle.md#Host-crash-reaper
use std::sync::atomic::{AtomicU8, Ordering};

pub enum HostReaperKind {
    LinuxPdeathsig,
    DarwinKqueue,
    WindowsJobObject,
}

pub struct HostPlane {
    reaper_kind: HostReaperKind,
    pressure_level: AtomicU8,
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
                // NOTE: fallback for other targets (covers Android): pdeathsig family.
                HostReaperKind::LinuxPdeathsig
            }
        };
        Self {
            reaper_kind,
            pressure_level: AtomicU8::new(0),
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

    pub fn poll_pressure_events(&self) -> Result<(), kelir_interface::errors::PluginError> {
        // TODO: sample host memory pressure and update pressure_level.
        Ok(())
    }

    pub fn arm_host_reaper(
        &self,
        parent_pid: u32,
    ) -> Result<(), kelir_interface::errors::PluginError> {
        // TODO: arm the platform reaper (pdeathsig / kqueue NOTE_EXIT / Job Object) on parent_pid.
        let _ = parent_pid;
        Ok(())
    }
}
