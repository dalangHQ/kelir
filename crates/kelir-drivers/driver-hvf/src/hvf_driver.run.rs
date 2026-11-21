// see: cans/01-architecture.md#Driver-backends
use kelir_interface::driver_interface::{VmmDriver, VmmError};

use crate::HvfDriver;

// TODO (L3) [hd_04_cold_boot_budget_enforcement]: remove this allow when the live vCPU loop calls run_vcpu and promote_chunked_stage2_pages; slop: allow masks the unwired vCPU loop while the build stays green.
#[allow(dead_code)]
impl HvfDriver {
    // TODO (L3) [hd_04_cold_boot_budget_enforcement]: call sys_driver::hv_vm_create(0) inside an unsafe block, then sys_driver::hv_vcpu_create per vCPU; slop: boot Ok without hv_vm_create, 50ms budget never measured
    pub(crate) fn create_vm(&mut self) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "hvf boot: hv_vm_create + hv_vcpu_create".into(),
        ))
    }

    // TODO (L3) [hd_04_cold_boot_budget_enforcement]: call sys_driver::hv_vcpu_run on each vcpu handle inside an unsafe block; slop: vCPU never runs yet boot reports first exec within budget
    pub(crate) fn run_vcpu(&mut self) -> Result<(), VmmError> {
        Err(VmmError::Unsupported("hvf run: hv_vcpu_run".into()))
    }

    // TODO (L3) [lc_01_cow_fork_warm_memory_sharing]: in-process Stage-2 CoW fork via sys_driver::hv_vm_protect, never an OS fork() call on multi-threaded hosts; slop: OS fork() in a multi-threaded Tokio/NAPI host SIGABRTs before the child runs, and fork shares pages without Stage-2 write protection, child mutates parent
    pub(crate) fn protect_fork(&self) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "hvf fork: hv_vm_protect CoW fork".into(),
        ))
    }

    // see: cans/08-lifecycle.md#CoW-fork
    // TODO (L3) [hd_06_hvf_stage2_chunk_promoter]: promote 2MB contiguous blocks via Mach vm_copy CoW; slop: 4KB pages promoted as 2MB chunks, chunk size never asserted
    pub(crate) fn promote_chunked_stage2_pages(&self, _ipa: u64) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "hvf stage-2 promotion: Mach vm_copy CoW".into(),
        ))
    }

    // TODO (L3) [lc_01_cow_fork_warm_memory_sharing]: clone VM state via in-process hv_vm_protect Stage-2 CoW fork in <15ms per cans/artifacts/code/vmm-driver-trait.rs, never an OS fork() call; slop: OS fork() in a multi-threaded host aborts, child gets fresh state, warm parent memory not inherited
    pub(crate) fn clone_fork_child(&self) -> Result<Box<dyn VmmDriver>, VmmError> {
        Err(VmmError::Unsupported(
            "hvf fork: CoW child driver".into(),
        ))
    }
}
