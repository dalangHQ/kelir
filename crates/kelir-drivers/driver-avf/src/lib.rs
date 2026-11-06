#![forbid(unsafe_code)]
// see: cans/01-architecture.md#Driver-backends
// see: cans/artifacts/diagrams/host-runtime.txt
use kelir_interface::driver::{SnapshotDescriptor, VmConfig, VmmDriver, VmmError};

#[allow(dead_code)]
pub struct AvfDriver {
    avf_fd: i32,
    config: Option<VmConfig>,
}

impl AvfDriver {
    pub fn new() -> Result<Self, VmmError> {
        Ok(Self {
            avf_fd: -1,
            config: None,
        })
    }
}

impl VmmDriver for AvfDriver {
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
        // see: cans/01-architecture.md#VmmDriver-trait
        Ok(Box::new(Self::new()?))
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

    fn set_vcpus(&mut self, _count: u8) -> Result<(), VmmError> {
        // see: cans/01-architecture.md#VmmDriver-trait
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
