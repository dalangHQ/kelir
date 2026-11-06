#![forbid(unsafe_code)]
// see: cans/01-architecture.md#Driver-backends
// see: cans/08-lifecycle.md#CoW-fork
// see: cans/artifacts/diagrams/host-runtime.txt
// TODO (L3): tests/test_cow_fork.rs + test_rollback.rs per cans/artifacts/config/project-structure.txt#kelir-drivers.
use kelir_interface::driver::{SnapshotDescriptor, VmConfig, VmmDriver, VmmError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverState {
    Unbooted,
    Booted,
    Paused,
}

// see: cans/01-architecture.md#Boot-budget
// see: cans/08-lifecycle.md#Rollback
// see: cans/08-lifecycle.md#CoW-fork
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriverCapabilities {
    pub backend: &'static str,
    pub cold_boot_budget_ms: u64,
    pub rollback_budget_ms: u64,
    pub fork_budget_ms: u64,
    pub requires_hardware_vmm: bool,
    pub supports_cow_fork: bool,
}

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
    state: DriverState,
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
            state: DriverState::Unbooted,
        })
    }

    pub fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities {
            backend: "rustvmm",
            cold_boot_budget_ms: 40,
            rollback_budget_ms: 8,
            fork_budget_ms: 10,
            requires_hardware_vmm: true,
            supports_cow_fork: true,
        }
    }

    fn ensure_booted(&self) -> Result<(), VmmError> {
        match self.state {
            DriverState::Booted | DriverState::Paused => Ok(()),
            DriverState::Unbooted => Err(VmmError::InvalidState(
                "rustvmm driver is not booted (L3)".into(),
            )),
        }
    }

    fn init_backend(&mut self) -> Result<(), VmmError> {
        // TODO (L3): initialize KVM, VM, memory slots, and vCPUs via KVM_CREATE_VM.
        Err(VmmError::Unsupported(
            "rustvmm boot: KVM_CREATE_VM init (L3)".into(),
        ))
    }

    #[cfg(target_os = "linux")]
    #[allow(dead_code)]
    fn filter_synthetic_cpuid(&self, _vcpu: &kvm_ioctls::VcpuFd) -> Result<(), VmmError> {
        // see: cans/01-architecture.md#Driver-backends
        // TODO (L3): enforce invariant TSC and Leaf 0x1 TSC-deadline.
        Err(VmmError::Unsupported(
            "rustvmm cpuid filter: synthetic CPUID enforcement (L3)".into(),
        ))
    }
}

impl VmmDriver for RustVmmDriver {
    fn boot(&mut self, config: &VmConfig) -> Result<(), VmmError> {
        self.config = Some(config.clone());
        self.init_backend()
    }

    fn pause(&mut self) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): pause vCPUs via KVM.
        Err(VmmError::Unsupported(
            "rustvmm pause: KVM vCPU pause (L3)".into(),
        ))
    }

    fn resume(&mut self) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): resume vCPUs via KVM.
        Err(VmmError::Unsupported(
            "rustvmm resume: KVM vCPU resume (L3)".into(),
        ))
    }

    fn step(&mut self) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): single-step one vCPU exit.
        Err(VmmError::Unsupported(
            "rustvmm step: KVM single-step (L3)".into(),
        ))
    }

    fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError> {
        self.ensure_booted()?;
        // see: cans/08-lifecycle.md#CoW-fork
        // TODO (L3): clone memory state via userfaultfd copy-on-write.
        Err(VmmError::Unsupported(
            "rustvmm fork: userfaultfd CoW fork (L3)".into(),
        ))
    }

    fn rollback(&mut self, _snapshot: &SnapshotDescriptor) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // see: cans/08-lifecycle.md#Rollback
        // TODO (L3): reset vCPU registers and revert userfaultfd memory tracking in <8ms.
        Err(VmmError::Unsupported(
            "rustvmm rollback: register reset + memory revert (L3)".into(),
        ))
    }

    fn snapshot(&mut self) -> Result<SnapshotDescriptor, VmmError> {
        self.ensure_booted()?;
        // TODO (L3): capture dirty byte pages and register states.
        Err(VmmError::Unsupported(
            "rustvmm snapshot: dirty-page + register capture (L3)".into(),
        ))
    }

    fn set_vcpus(&mut self, _count: u8) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): create or destroy vCPU fds to match count.
        Err(VmmError::Unsupported(
            "rustvmm set_vcpus: vCPU fd resize (L3)".into(),
        ))
    }

    fn adjust_balloon(&mut self, _target_mb: u32) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): deflate/inflate virtio-balloon device descriptor.
        Err(VmmError::Unsupported(
            "rustvmm adjust_balloon: virtio-balloon resize (L3)".into(),
        ))
    }

    fn skew_clock(&mut self, _offset_ns: i64) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // see: cans/08-lifecycle.md#Clock-skew
        // TODO (L3): adjust KVM_SET_CLOCK offset.
        Err(VmmError::Unsupported(
            "rustvmm skew_clock: KVM_SET_CLOCK offset (L3)".into(),
        ))
    }
}
