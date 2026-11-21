// see: cans/01-architecture.md#Driver-backends
use kelir_interface::driver_interface::VmmError;

use crate::RustVmmDriver;

// TODO (L3) [hd_11_cpuid_tsc_deadline_normalization]: remove this allow when the live vCPU loop calls filter_synthetic_cpuid and reads the KVM fds; slop: allow masks the unwired CPUID filter while the build stays green.
#[allow(dead_code)]
impl RustVmmDriver {
    // TODO (L3) [hd_04_cold_boot_budget_enforcement]: initialize KVM, VM, memory slots, and vCPUs via KVM_CREATE_VM; slop: boot Ok without KVM_CREATE_VM, 50ms budget never measured
    pub(crate) fn init_backend(&mut self) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "rustvmm boot: KVM_CREATE_VM init".into(),
        ))
    }

    // see: cans/01-architecture.md#Driver-backends
    // TODO (L3) [hd_11_cpuid_tsc_deadline_normalization]: enforce invariant TSC and Leaf 0x1 TSC-deadline; slop: guest CPUID passes host leaves through, TSC-deadline bit unset
    #[cfg(target_os = "linux")]
    pub(crate) fn filter_synthetic_cpuid(&self, _vcpu: &kvm_ioctls::VcpuFd) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "rustvmm cpuid filter: synthetic CPUID enforcement".into(),
        ))
    }
}
