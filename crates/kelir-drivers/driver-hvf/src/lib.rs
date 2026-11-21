// TODO: [hd_04_cold_boot_budget_enforcement]: call run_vcpu/promote_chunked_stage2_pages from the live vCPU loop; slop: vCPU never runs (deferred to L3)
#![allow(dead_code)]
// see: cans/01-architecture.md#Host-memory-pressure-watcher
// see: cans/08-lifecycle.md#CoW-fork
// see: cans/artifacts/diagrams/host-runtime.txt#driver-hvf
#[path = "sys_driver.rs"]
pub mod sys_driver;

use kelir_interface::driver_interface::{EntropyBackend, VmConfig};

#[path = "hvf_driver.flow.rs"]
pub mod hvf_driver;

pub struct HvfDriver {
    vcpu_ids: Vec<sys_driver::hv_vcpu_t>,
    memory_mapped_bytes: usize,
    config: Option<VmConfig>,
    entropy_backend: Option<EntropyBackend>,
    entropy_offset: u64,
}
