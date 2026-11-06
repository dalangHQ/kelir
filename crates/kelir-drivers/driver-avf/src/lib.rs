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
        // TODO: Connect /dev/avf or pKVM node
        Ok(())
    }

    fn pause(&mut self) -> Result<(), VmmError> {
        // TODO: Pause vCPUs via AVF
        Ok(())
    }

    fn resume(&mut self) -> Result<(), VmmError> {
        // TODO: Resume vCPUs via AVF
        Ok(())
    }

    fn step(&mut self) -> Result<(), VmmError> {
        // TODO: Single-step one vCPU exit
        Ok(())
    }

    fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError> {
        // TODO: Clone VM state via AVF
        Ok(Box::new(Self::new()?))
    }

    fn rollback(&mut self, _snapshot: &SnapshotDescriptor) -> Result<(), VmmError> {
        // TODO: Reset vCPU registers and revert memory tracking in <8ms
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
        // TODO: Create or destroy vCPUs to match count
        Ok(())
    }

    fn adjust_balloon(&mut self, _target_mb: u32) -> Result<(), VmmError> {
        // TODO: Deflate/inflate virtio-balloon device descriptor
        Ok(())
    }

    fn skew_clock(&mut self, _offset_ns: i64) -> Result<(), VmmError> {
        // TODO: Adjust pKVM virtual clock offset
        Ok(())
    }
}
