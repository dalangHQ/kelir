#![forbid(unsafe_code)]
// see: cans/01-architecture.md#Driver-backends
use kelir_interface::driver::{SnapshotDescriptor, VmConfig, VmmDriver, VmmError};

#[allow(dead_code)]
pub struct ProotDriver {
    allow_emulated_fallback: bool,
    config: Option<VmConfig>,
}

impl ProotDriver {
    pub fn new(allow_emulated_fallback: bool) -> Result<Self, VmmError> {
        // NOTE: the work-order names DriverError::ENOHARDWAREVMM here, but the artifact has no such variant; VmmError::Unsupported is the equivalent.
        if !allow_emulated_fallback {
            return Err(VmmError::Unsupported(
                "emulated fallback not allowed".to_string(),
            ));
        }
        Ok(Self {
            allow_emulated_fallback,
            config: None,
        })
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
        // TODO: Initialize proot ptrace tracing harness
        Ok(())
    }

    fn pause(&mut self) -> Result<(), VmmError> {
        // TODO: SIGSTOP the traced child
        Ok(())
    }

    fn resume(&mut self) -> Result<(), VmmError> {
        // TODO: SIGCONT the traced child
        Ok(())
    }

    fn step(&mut self) -> Result<(), VmmError> {
        // TODO: Single-step one traced syscall
        Ok(())
    }

    fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError> {
        Ok(Box::new(Self::new(self.allow_emulated_fallback)?))
    }

    fn rollback(&mut self, _snapshot: &SnapshotDescriptor) -> Result<(), VmmError> {
        // TODO: Reset child state and revert memory tracking in <8ms
        Ok(())
    }

    fn snapshot(&mut self) -> Result<SnapshotDescriptor, VmmError> {
        // TODO: Capture dirty byte pages and register states
        Ok(SnapshotDescriptor {
            id: String::new(),
            dirty_bytes: 0,
            created_at_ns: 0,
            parent_digest: None,
            guest_clock: None,
        })
    }

    fn set_vcpus(&mut self, count: u8) -> Result<(), VmmError> {
        // TODO: Emulated vCPU count is fixed at 1; count > 1 is unsupported
        if count > 1 {
            return Err(VmmError::Unsupported(
                "emulated vCPU count is fixed at 1".into(),
            ));
        }
        Ok(())
    }

    fn adjust_balloon(&mut self, _target_mb: u32) -> Result<(), VmmError> {
        // TODO: Adjust emulated memory ceiling
        Ok(())
    }

    fn skew_clock(&mut self, _offset_ns: i64) -> Result<(), VmmError> {
        // TODO: Adjust emulated clock offset
        Ok(())
    }
}
