#![forbid(unsafe_code)]
// TODO: [lc_12_host_reaper_pdeathsig_enforcement]: read BwrapDriver.child_pid once the supervisor reaps the bwrap child; slop: child pid stored but never read (deferred to L3)
#![allow(dead_code)]
// see: cans/01-architecture.md#Driver-backends
// see: cans/artifacts/diagrams/host-runtime.txt#driver-bwrap
use kelir_interface::driver_interface::{EntropyBackend, VmConfig};

#[path = "bwrap_driver.flow.rs"]
pub mod bwrap_driver;
#[path = "bwrap_driver.run.rs"]
pub mod bwrap_driver_run;

pub struct BwrapDriver {
    allow_emulated_fallback: bool,
    child_pid: Option<libc::pid_t>,
    config: Option<VmConfig>,
    entropy_backend: Option<EntropyBackend>,
    entropy_offset: u64,
}
