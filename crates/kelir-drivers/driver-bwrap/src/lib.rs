#![forbid(unsafe_code)]
// TODO (L3) [lc_12_host_reaper_pdeathsig_enforcement]: read BwrapDriver.child_pid once the supervisor reaps the bwrap child; slop: child pid stored but never read
#![allow(dead_code)]
// see: cans/01-architecture.md#Driver-backends
// see: cans/artifacts/diagrams/host-runtime.txt#driver-bwrap
use kelir_interface::driver_interface::{
    DriverCapabilities, EntropyBackend, EntropyStreamPosition, VmConfig, VmmDriver, VmmError,
    check_hw_lockfile, reject_emulated_deterministic_clock,
};

pub struct BwrapDriver {
    allow_emulated_fallback: bool,
    child_pid: Option<libc::pid_t>,
    config: Option<VmConfig>,
    entropy_backend: Option<EntropyBackend>,
    entropy_offset: u64,
}
#[path = "driver.run.rs"]
mod driver_impl;
