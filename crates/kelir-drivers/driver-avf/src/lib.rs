#![forbid(unsafe_code)]
// see: cans/01-architecture.md#Driver-backends
// see: cans/artifacts/diagrams/host-runtime.txt
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
pub struct AvfDriver {
    avf_fd: i32,
    config: Option<VmConfig>,
    state: DriverState,
}

impl AvfDriver {
    pub fn new() -> Result<Self, VmmError> {
        Ok(Self {
            avf_fd: -1,
            config: None,
            state: DriverState::Unbooted,
        })
    }

    pub fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities {
            backend: "avf",
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
                "avf driver is not booted (L3)".into(),
            )),
        }
    }

    fn open_node(&mut self) -> Result<(), VmmError> {
        // TODO (L3): connect /dev/avf or the pKVM node.
        Err(VmmError::Unsupported("avf boot: /dev/avf open (L3)".into()))
    }
}

impl VmmDriver for AvfDriver {
    fn boot(&mut self, config: &VmConfig) -> Result<(), VmmError> {
        self.config = Some(config.clone());
        self.open_node()
    }

    fn pause(&mut self) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): pause vCPUs via AVF.
        Err(VmmError::Unsupported(
            "avf pause: AVF vCPU pause (L3)".into(),
        ))
    }

    fn resume(&mut self) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): resume vCPUs via AVF.
        Err(VmmError::Unsupported(
            "avf resume: AVF vCPU resume (L3)".into(),
        ))
    }

    fn step(&mut self) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): single-step one vCPU exit.
        Err(VmmError::Unsupported(
            "avf step: AVF single-step (L3)".into(),
        ))
    }

    fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError> {
        self.ensure_booted()?;
        // TODO (L3): clone VM state via AVF.
        Err(VmmError::Unsupported("avf fork: AVF VM clone (L3)".into()))
    }

    fn rollback(&mut self, _snapshot: &SnapshotDescriptor) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): reset vCPU registers and revert memory tracking in <8ms.
        Err(VmmError::Unsupported(
            "avf rollback: register reset + memory revert (L3)".into(),
        ))
    }

    fn snapshot(&mut self) -> Result<SnapshotDescriptor, VmmError> {
        self.ensure_booted()?;
        // TODO (L3): capture dirty byte pages and register states.
        Err(VmmError::Unsupported(
            "avf snapshot: dirty-page + register capture (L3)".into(),
        ))
    }

    fn set_vcpus(&mut self, _count: u8) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): create or destroy vCPUs to match count.
        Err(VmmError::Unsupported(
            "avf set_vcpus: vCPU resize (L3)".into(),
        ))
    }

    fn adjust_balloon(&mut self, _target_mb: u32) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): deflate/inflate virtio-balloon device descriptor.
        Err(VmmError::Unsupported(
            "avf adjust_balloon: virtio-balloon resize (L3)".into(),
        ))
    }

    fn skew_clock(&mut self, _offset_ns: i64) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): adjust pKVM virtual clock offset.
        Err(VmmError::Unsupported(
            "avf skew_clock: pKVM clock offset (L3)".into(),
        ))
    }
}
