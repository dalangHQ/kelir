#![forbid(unsafe_code)]
// see: cans/01-architecture.md#Driver-backends
use kelir_interface::driver::{SnapshotDescriptor, VmConfig, VmmDriver, VmmError};

#[allow(dead_code)]
pub struct BwrapDriver {
    allow_emulated_fallback: bool,
    child_pid: Option<libc::pid_t>,
    config: Option<VmConfig>,
}

impl BwrapDriver {
    pub fn new(allow_emulated_fallback: bool) -> Result<Self, VmmError> {
        if !allow_emulated_fallback {
            return Err(VmmError::Unsupported(
                "emulated fallback not allowed".to_string(),
            ));
        }
        Ok(Self {
            allow_emulated_fallback,
            child_pid: None,
            config: None,
        })
    }
}

impl VmmDriver for BwrapDriver {
    fn boot(&mut self, config: &VmConfig) -> Result<(), VmmError> {
        if !self.allow_emulated_fallback {
            return Err(VmmError::Unsupported(
                "emulated fallback not allowed".to_string(),
            ));
        }
        self.config = Some(config.clone());
        // TODO: Spawn bubblewrap/unshare user namespace via raw libc
        Ok(())
    }

    fn pause(&mut self) -> Result<(), VmmError> {
        // TODO: SIGSTOP the child namespace
        Ok(())
    }

    fn resume(&mut self) -> Result<(), VmmError> {
        // TODO: SIGCONT the child namespace
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

    fn set_vcpus(&mut self, _count: u8) -> Result<(), VmmError> {
        // TODO: Emulated vCPU count is fixed at 1; count > 1 is unsupported
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
