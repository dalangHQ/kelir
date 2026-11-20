#![forbid(unsafe_code)]
// see: cans/01-architecture.md#Driver-backends
// see: cans/artifacts/diagrams/host-runtime.txt#driver-avf
use kelir_interface::driver_interface::{EntropyBackend, VmConfig};

#[path = "avf_driver.flow.rs"]
pub mod avf_driver;

pub struct AvfDriver {
    avf_fd: i32,
    config: Option<VmConfig>,
    entropy_backend: Option<EntropyBackend>,
    entropy_offset: u64,
}
