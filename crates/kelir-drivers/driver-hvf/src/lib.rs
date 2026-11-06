// see: cans/01-architecture.md#Host-memory-pressure-watcher
// see: cans/08-lifecycle.md#CoW-fork
// see: cans/artifacts/diagrams/host-runtime.txt
pub mod sys;

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
pub struct HvfDriver {
    vcpu_ids: Vec<sys::hv_vcpu_t>,
    memory_mapped_bytes: usize,
    config: Option<VmConfig>,
    state: DriverState,
}

impl HvfDriver {
    pub fn new() -> Result<Self, VmmError> {
        Ok(Self {
            vcpu_ids: Vec::new(),
            memory_mapped_bytes: 0,
            config: None,
            state: DriverState::Unbooted,
        })
    }

    pub fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities {
            backend: "hvf",
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
                "hvf driver is not booted (L3)".into(),
            )),
        }
    }

    fn create_vm(&mut self) -> Result<(), VmmError> {
        // TODO (L3): call sys::hv_vm_create(0) inside an unsafe block, then sys::hv_vcpu_create per vCPU.
        Err(VmmError::Unsupported(
            "hvf boot: hv_vm_create + hv_vcpu_create (L3)".into(),
        ))
    }

    fn run_vcpu(&mut self) -> Result<(), VmmError> {
        // TODO (L3): call sys::hv_vcpu_run on each vcpu handle inside an unsafe block.
        Err(VmmError::Unsupported("hvf run: hv_vcpu_run (L3)".into()))
    }

    fn protect_fork(&self) -> Result<(), VmmError> {
        // TODO (L3): Mach copy-on-write + Stage-2 write protection via sys::hv_vm_protect.
        Err(VmmError::Unsupported(
            "hvf fork: hv_vm_protect CoW fork (L3)".into(),
        ))
    }

    #[allow(dead_code)]
    fn promote_chunked_stage2_pages(&self, _ipa: u64) -> Result<(), VmmError> {
        // see: cans/08-lifecycle.md#CoW-fork
        // TODO (L3): promote 2MB contiguous blocks via Mach vm_copy CoW.
        Err(VmmError::Unsupported(
            "hvf stage-2 promotion: Mach vm_copy CoW (L3)".into(),
        ))
    }
}

impl VmmDriver for HvfDriver {
    fn boot(&mut self, config: &VmConfig) -> Result<(), VmmError> {
        self.config = Some(config.clone());
        self.create_vm()
    }

    fn pause(&mut self) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): pause vCPUs via hv_vcpu_run loop exit.
        Err(VmmError::Unsupported(
            "hvf pause: hv_vcpu_run loop exit (L3)".into(),
        ))
    }

    fn resume(&mut self) -> Result<(), VmmError> {
        self.ensure_booted()?;
        self.run_vcpu()
    }

    fn step(&mut self) -> Result<(), VmmError> {
        self.ensure_booted()?;
        self.run_vcpu()
    }

    fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError> {
        self.ensure_booted()?;
        // see: cans/08-lifecycle.md#CoW-fork
        self.protect_fork()?;
        // TODO (L3): return the CoW child driver once hv_vm_protect fork lands.
        Err(VmmError::Unsupported(
            "hvf fork: CoW child driver (L3)".into(),
        ))
    }

    fn rollback(&mut self, _snapshot: &SnapshotDescriptor) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // see: cans/08-lifecycle.md#Rollback
        // TODO (L3): reset Stage-2 translation table roots in <8ms.
        Err(VmmError::Unsupported(
            "hvf rollback: Stage-2 root reset (L3)".into(),
        ))
    }

    fn snapshot(&mut self) -> Result<SnapshotDescriptor, VmmError> {
        self.ensure_booted()?;
        // TODO (L3): capture dirty byte pages and register states.
        Err(VmmError::Unsupported(
            "hvf snapshot: dirty-page + register capture (L3)".into(),
        ))
    }

    fn set_vcpus(&mut self, _count: u8) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): create or destroy hv_vcpu_t handles to match count.
        Err(VmmError::Unsupported(
            "hvf set_vcpus: hv_vcpu_create / hv_vcpu_destroy (L3)".into(),
        ))
    }

    fn adjust_balloon(&mut self, _target_mb: u32) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // see: cans/01-architecture.md#Host-memory-pressure-watcher
        // TODO (L3): Stage-2 hv_vm_unmap + mach_vm_deallocate purge.
        Err(VmmError::Unsupported(
            "hvf adjust_balloon: hv_vm_unmap purge (L3)".into(),
        ))
    }

    fn skew_clock(&mut self, _offset_ns: i64) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // see: cans/08-lifecycle.md#Clock-skew
        // TODO (L3): adjust ARM64 CNTVOFF_EL2.
        Err(VmmError::Unsupported(
            "hvf skew_clock: CNTVOFF_EL2 offset (L3)".into(),
        ))
    }
}
