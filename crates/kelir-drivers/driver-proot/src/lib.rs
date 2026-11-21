#![forbid(unsafe_code)]
// see: cans/01-architecture.md#Driver-backends
// see: cans/artifacts/diagrams/host-runtime.txt#driver-proot
use kelir_interface::driver_interface::{EntropyBackend, VmConfig};

#[path = "proot_driver.flow.rs"]
pub mod proot_driver;
#[path = "proot_driver.run.rs"]
pub mod proot_driver_run;

pub struct ProotDriver {
    allow_emulated_fallback: bool,
    config: Option<VmConfig>,
    entropy_backend: Option<EntropyBackend>,
    entropy_offset: u64,
}
