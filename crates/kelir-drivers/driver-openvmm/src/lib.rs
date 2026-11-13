#![forbid(unsafe_code)]
// see: cans/01-architecture.md#Driver-backends
// see: cans/artifacts/diagrams/host-runtime.txt#driver-openvmm
// TODO (L2): Windows named-pipe fallback arrives with transport_connect (no HV_SOCKET, per standing decision).
use kelir_interface::driver_interface::{
    DriverCapabilities, EntropyBackend, EntropyStreamPosition, VmConfig, VmmDriver, VmmError,
    check_hw_lockfile,
};

pub struct OpenVmmDriver {
    partition_handle: usize,
    config: Option<VmConfig>,
    entropy_backend: Option<EntropyBackend>,
    entropy_offset: u64,
}
#[path = "driver.run.rs"]
mod driver_impl;
