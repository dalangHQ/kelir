#![forbid(unsafe_code)]
// see: cans/01-architecture.md#Driver-backends
// see: cans/08-lifecycle.md#CoW-fork
// see: cans/artifacts/diagrams/host-runtime.txt
use kelir_interface::driver::{SnapshotDescriptor, VmConfig, VmmDriver, VmmError};

#[allow(dead_code)]
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
}

impl RustVmmDriver {
    pub fn new() -> Result<Self, VmmError> {
        #[cfg(not(target_os = "linux"))]
        return Err(VmmError::Unsupported(
            "driver-rustvmm requires Linux".into(),
        ));
        #[cfg(target_os = "linux")]
        Ok(Self {
            kvm_fd: None,
            vm_fd: None,
            vcpu_fds: Vec::new(),
            uffd: None,
            config: None,
        })
    }

    #[cfg(target_os = "linux")]
    #[allow(dead_code)]
    fn filter_synthetic_cpuid(&self, _vcpu: &kvm_ioctls::VcpuFd) -> Result<(), VmmError> {
        // see: cans/01-architecture.md#Driver-backends
        Ok(())
    }
}

impl VmmDriver for RustVmmDriver {
    fn boot(&mut self, config: &VmConfig) -> Result<(), VmmError> {
        self.config = Some(config.clone());
        // see: cans/01-architecture.md#VmmDriver-trait
        Ok(())
    }

    fn pause(&mut self) -> Result<(), VmmError> {
        // see: cans/01-architecture.md#VmmDriver-trait
        Ok(())
    }

    fn resume(&mut self) -> Result<(), VmmError> {
        // see: cans/01-architecture.md#VmmDriver-trait
        Ok(())
    }

    fn step(&mut self) -> Result<(), VmmError> {
        // see: cans/01-architecture.md#VmmDriver-trait
        Ok(())
    }

    fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError> {
        // see: cans/08-lifecycle.md#CoW-fork
        Ok(Box::new(Self::new()?))
    }

    fn rollback(&mut self, _snapshot: &SnapshotDescriptor) -> Result<(), VmmError> {
        // see: cans/08-lifecycle.md#Rollback
        Ok(())
    }

    fn snapshot(&mut self) -> Result<SnapshotDescriptor, VmmError> {
        // see: cans/01-architecture.md#VmmDriver-trait
        Ok(SnapshotDescriptor {
            id: String::new(),
            dirty_bytes: 0,
            created_at_ns: 0,
            parent_digest: None,
            guest_clock: None,
        })
    }

    fn set_vcpus(&mut self, _count: u8) -> Result<(), VmmError> {
        // see: cans/01-architecture.md#VmmDriver-trait
        Ok(())
    }

    fn adjust_balloon(&mut self, _target_mb: u32) -> Result<(), VmmError> {
        // see: cans/01-architecture.md#VmmDriver-trait
        Ok(())
    }

    fn skew_clock(&mut self, _offset_ns: i64) -> Result<(), VmmError> {
        // see: cans/08-lifecycle.md#Clock-skew
        Ok(())
    }
}
