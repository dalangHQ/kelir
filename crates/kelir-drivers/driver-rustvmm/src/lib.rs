#![forbid(unsafe_code)]
// TODO (L3) [hd_11_cpuid_tsc_deadline_normalization]: call filter_synthetic_cpuid from the live vCPU loop and read the KVM fds; slop: CPUID filter and fds never used
#![allow(dead_code)]
// see: cans/01-architecture.md#Driver-backends
// see: cans/08-lifecycle.md#CoW-fork
// see: cans/artifacts/diagrams/host-runtime.txt#driver-rustvmm
// TODO (L3) [hd_03_hardware_rollback_timing_bound]: tests/test_cow_fork.rs + test_rollback.rs per cans/artifacts/config/project-structure.txt#kelir-drivers; slop: rollback test absent, 8ms reset bound never measured
pub mod host_probe;

use kelir_interface::driver_interface::{
    DriverCapabilities, EntropyBackend, EntropyStreamPosition, VmConfig, VmmDriver, VmmError,
    check_hw_lockfile,
};

pub struct RustVmmDriver {
    #[cfg(target_os = "linux")]
    kvm_fd: Option<kvm_ioctls::Kvm>,
    #[cfg(target_os = "linux")]
    vm_fd: Option<kvm_ioctls::VmFd>,
    #[cfg(target_os = "linux")]
    vcpu_fds: Vec<kvm_ioctls::VcpuFd>,
    #[cfg(target_os = "linux")]
    uffd: Option<userfaultfd::Uffd>,
    config: Option<VmConfig>,
    entropy_backend: Option<EntropyBackend>,
    entropy_offset: u64,
}
#[path = "driver.run.rs"]
mod driver_impl;
