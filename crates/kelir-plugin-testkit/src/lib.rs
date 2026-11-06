// see: cans/02-plugin-model.md#Plugin-testkit
#![forbid(unsafe_code)]

pub mod assertions;
pub mod fixtures;
pub mod handlers;
pub mod mock_bus;

use kelir_interface::driver::{SnapshotDescriptor, VmConfig, VmmDriver};
use kelir_interface::errors::DriverError;

pub struct MockVmmDriver;

impl VmmDriver for MockVmmDriver {
    fn boot(&mut self, _config: &VmConfig) -> Result<(), DriverError> {
        Ok(())
    }
    fn pause(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
    fn resume(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
    fn step(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
    fn fork(&self) -> Result<Box<dyn VmmDriver>, DriverError> {
        Ok(Box::new(MockVmmDriver))
    }
    fn rollback(&mut self, _snapshot: &SnapshotDescriptor) -> Result<(), DriverError> {
        Ok(())
    }
    fn snapshot(&mut self) -> Result<SnapshotDescriptor, DriverError> {
        Ok(SnapshotDescriptor {
            id: String::new(),
            dirty_bytes: 0,
            created_at_ns: 0,
            parent_digest: None,
            guest_clock: None,
        })
    }
    fn set_vcpus(&mut self, _count: u8) -> Result<(), DriverError> {
        Ok(())
    }
    fn adjust_balloon(&mut self, _target_mb: u32) -> Result<(), DriverError> {
        Ok(())
    }
    fn skew_clock(&mut self, _offset_ns: i64) -> Result<(), DriverError> {
        Ok(())
    }
}
