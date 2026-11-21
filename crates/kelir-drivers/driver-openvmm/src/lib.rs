#![forbid(unsafe_code)]
// see: cans/01-architecture.md#Driver-backends
// see: cans/artifacts/diagrams/host-runtime.txt#driver-openvmm
// TODO (L2): Windows named-pipe fallback arrives with transport_connect (no HV_SOCKET, per standing decision).
use kelir_interface::driver_interface::{EntropyBackend, VmConfig};

#[path = "openvmm_driver.flow.rs"]
pub mod openvmm_driver;
#[path = "openvmm_driver.run.rs"]
pub mod openvmm_driver_run;

pub struct OpenVmmDriver {
    partition_handle: usize,
    config: Option<VmConfig>,
    entropy_backend: Option<EntropyBackend>,
    entropy_offset: u64,
}
