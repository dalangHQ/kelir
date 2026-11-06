#![forbid(unsafe_code)]
// see: cans/01-architecture.md#Driver-backends
// see: cans/artifacts/diagrams/host-runtime.txt
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
        Ok(Box::new(Self::new(self.allow_emulated_fallback)?))
    }

    fn rollback(&mut self, _snapshot: &SnapshotDescriptor) -> Result<(), VmmError> {
        // see: cans/01-architecture.md#VmmDriver-trait
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

    fn set_vcpus(&mut self, count: u8) -> Result<(), VmmError> {
        if count > 1 {
            return Err(VmmError::Unsupported(
                "emulated vCPU count is fixed at 1".into(),
            ));
        }
        Ok(())
    }

    fn adjust_balloon(&mut self, _target_mb: u32) -> Result<(), VmmError> {
        // see: cans/01-architecture.md#VmmDriver-trait
        Ok(())
    }

    fn skew_clock(&mut self, _offset_ns: i64) -> Result<(), VmmError> {
        // see: cans/01-architecture.md#VmmDriver-trait
        Ok(())
    }
}
