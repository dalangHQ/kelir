// TODO (L3) [hd_04_cold_boot_budget_enforcement]: call run_vcpu/promote_chunked_stage2_pages from the live vCPU loop; slop: vCPU never runs
#![allow(dead_code)]
// see: cans/01-architecture.md#Host-memory-pressure-watcher
// see: cans/08-lifecycle.md#CoW-fork
// see: cans/artifacts/diagrams/host-runtime.txt#driver-hvf
#[path = "sys_driver.run.rs"]
pub mod sys_driver;

use kelir_interface::driver_interface::{
    DriverCapabilities, EntropyBackend, EntropyStreamPosition, VmConfig, VmmDriver, VmmError,
    check_hw_lockfile,
};

pub struct HvfDriver {
    vcpu_ids: Vec<sys_driver::hv_vcpu_t>,
    memory_mapped_bytes: usize,
    config: Option<VmConfig>,
    entropy_backend: Option<EntropyBackend>,
    entropy_offset: u64,
}
#[path = "driver.run.rs"]
mod driver_impl;
