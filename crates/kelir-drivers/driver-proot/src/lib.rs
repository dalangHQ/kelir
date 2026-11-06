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
pub struct ProotDriver {
    allow_emulated_fallback: bool,
    config: Option<VmConfig>,
    state: DriverState,
}

impl ProotDriver {
    pub fn new(allow_emulated_fallback: bool) -> Result<Self, VmmError> {
        if !allow_emulated_fallback {
            return Err(VmmError::Unsupported(
                "emulated fallback not allowed".to_string(),
            ));
        }
        Ok(Self {
            allow_emulated_fallback,
            config: None,
            state: DriverState::Unbooted,
        })
    }

    pub fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities {
            backend: "proot",
            cold_boot_budget_ms: 40,
            rollback_budget_ms: 8,
            fork_budget_ms: 10,
            requires_hardware_vmm: false,
            supports_cow_fork: false,
        }
    }

    fn ensure_booted(&self) -> Result<(), VmmError> {
        match self.state {
            DriverState::Booted | DriverState::Paused => Ok(()),
            DriverState::Unbooted => Err(VmmError::InvalidState(
                "proot driver is not booted (L3)".into(),
            )),
        }
    }

    fn init_tracer(&mut self) -> Result<(), VmmError> {
        // TODO (L3): initialize proot ptrace tracing harness.
        Err(VmmError::Unsupported(
            "proot boot: ptrace tracing harness init (L3)".into(),
        ))
    }
}

impl VmmDriver for ProotDriver {
    fn boot(&mut self, config: &VmConfig) -> Result<(), VmmError> {
        if !self.allow_emulated_fallback {
            return Err(VmmError::Unsupported(
                "emulated fallback not allowed".to_string(),
            ));
        }
        self.config = Some(config.clone());
        self.init_tracer()
    }

    fn pause(&mut self) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): SIGSTOP the traced child.
        Err(VmmError::Unsupported(
            "proot pause: SIGSTOP traced child (L3)".into(),
        ))
    }

    fn resume(&mut self) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): SIGCONT the traced child.
        Err(VmmError::Unsupported(
            "proot resume: SIGCONT traced child (L3)".into(),
        ))
    }

    fn step(&mut self) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): single-step one traced syscall.
        Err(VmmError::Unsupported(
            "proot step: traced syscall single-step (L3)".into(),
        ))
    }

    fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError> {
        self.ensure_booted()?;
        // TODO (L3): fork the traced child.
        Err(VmmError::Unsupported(
            "proot fork: traced child fork (L3)".into(),
        ))
    }

    fn rollback(&mut self, _snapshot: &SnapshotDescriptor) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): reset child state and revert memory tracking in <8ms.
        Err(VmmError::Unsupported(
            "proot rollback: child state reset (L3)".into(),
        ))
    }

    fn snapshot(&mut self) -> Result<SnapshotDescriptor, VmmError> {
        self.ensure_booted()?;
        // TODO (L3): capture dirty byte pages and register states.
        Err(VmmError::Unsupported(
            "proot snapshot: dirty-page + register capture (L3)".into(),
        ))
    }

    fn set_vcpus(&mut self, count: u8) -> Result<(), VmmError> {
        // TODO (L3): emulated vCPU count is fixed at 1; count > 1 is unsupported.
        if count > 1 {
            return Err(VmmError::Unsupported(
                "emulated vCPU count is fixed at 1".into(),
            ));
        }
        self.ensure_booted()?;
        Err(VmmError::Unsupported(
            "proot set_vcpus: emulated vCPU resize (L3)".into(),
        ))
    }

    fn adjust_balloon(&mut self, _target_mb: u32) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): adjust emulated memory ceiling.
        Err(VmmError::Unsupported(
            "proot adjust_balloon: emulated memory ceiling (L3)".into(),
        ))
    }

    fn skew_clock(&mut self, _offset_ns: i64) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): adjust emulated clock offset.
        Err(VmmError::Unsupported(
            "proot skew_clock: emulated clock offset (L3)".into(),
        ))
    }
}
