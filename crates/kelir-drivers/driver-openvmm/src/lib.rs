#![forbid(unsafe_code)]
// see: cans/01-architecture.md#Driver-backends
// see: cans/artifacts/diagrams/host-runtime.txt
// TODO (L2): Windows named-pipe fallback arrives with transport_connect (no HV_SOCKET, per standing decision).
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
pub struct OpenVmmDriver {
    partition_handle: usize,
    config: Option<VmConfig>,
    state: DriverState,
}

impl OpenVmmDriver {
    pub fn new() -> Result<Self, VmmError> {
        Ok(Self {
            partition_handle: 0,
            config: None,
            state: DriverState::Unbooted,
        })
    }

    pub fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities {
            backend: "openvmm",
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
                "openvmm driver is not booted (L3)".into(),
            )),
        }
    }

    fn create_partition(&mut self) -> Result<(), VmmError> {
        // TODO (L3): create the WHP partition via WHvCreatePartition and commit Local APIC state before the TSC deadline.
        Err(VmmError::Unsupported(
            "openvmm boot: WHvCreatePartition (L3)".into(),
        ))
    }
}

impl VmmDriver for OpenVmmDriver {
    fn boot(&mut self, config: &VmConfig) -> Result<(), VmmError> {
        self.config = Some(config.clone());
        self.create_partition()
    }

    fn pause(&mut self) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): pause vCPUs via WHP.
        Err(VmmError::Unsupported(
            "openvmm pause: WHP vCPU pause (L3)".into(),
        ))
    }

    fn resume(&mut self) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): resume vCPUs via WHP.
        Err(VmmError::Unsupported(
            "openvmm resume: WHP vCPU resume (L3)".into(),
        ))
    }

    fn step(&mut self) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): single-step one vCPU exit.
        Err(VmmError::Unsupported(
            "openvmm step: WHP single-step (L3)".into(),
        ))
    }

    fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError> {
        self.ensure_booted()?;
        // TODO (L3): clone partition state via WHP.
        Err(VmmError::Unsupported(
            "openvmm fork: WHP partition clone (L3)".into(),
        ))
    }

    fn rollback(&mut self, _snapshot: &SnapshotDescriptor) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): reset vCPU registers and revert memory tracking in <8ms.
        Err(VmmError::Unsupported(
            "openvmm rollback: register reset + memory revert (L3)".into(),
        ))
    }

    fn snapshot(&mut self) -> Result<SnapshotDescriptor, VmmError> {
        self.ensure_booted()?;
        // TODO (L3): capture dirty byte pages and register states.
        Err(VmmError::Unsupported(
            "openvmm snapshot: dirty-page + register capture (L3)".into(),
        ))
    }

    fn set_vcpus(&mut self, _count: u8) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): create or destroy WHP vCPUs to match count.
        Err(VmmError::Unsupported(
            "openvmm set_vcpus: WHP vCPU resize (L3)".into(),
        ))
    }

    fn adjust_balloon(&mut self, _target_mb: u32) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): deflate/inflate virtio-balloon device descriptor.
        Err(VmmError::Unsupported(
            "openvmm adjust_balloon: virtio-balloon resize (L3)".into(),
        ))
    }

    fn skew_clock(&mut self, _offset_ns: i64) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): adjust WHP virtual clock offset.
        Err(VmmError::Unsupported(
            "openvmm skew_clock: WHP clock offset (L3)".into(),
        ))
    }
}
