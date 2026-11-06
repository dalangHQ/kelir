#![forbid(unsafe_code)]
// see: cans/01-architecture.md#Driver-backends
// see: cans/artifacts/diagrams/host-runtime.txt
// TODO (L2): Windows named-pipe fallback arrives with transport_connect (no HV_SOCKET, per standing decision).
use kelir_interface::driver::{SnapshotDescriptor, VmConfig, VmmDriver, VmmError};

#[allow(dead_code)]
pub struct OpenVmmDriver {
    partition_handle: usize,
    config: Option<VmConfig>,
}

impl OpenVmmDriver {
    pub fn new() -> Result<Self, VmmError> {
        Ok(Self {
            partition_handle: 0,
            config: None,
        })
    }
}

impl VmmDriver for OpenVmmDriver {
    fn boot(&mut self, config: &VmConfig) -> Result<(), VmmError> {
        self.config = Some(config.clone());
        // TODO: Setup WHP partition and commit Local APIC state before TSC deadline
        Ok(())
    }

    fn pause(&mut self) -> Result<(), VmmError> {
        // TODO: Pause vCPUs via WHP
        Ok(())
    }

    fn resume(&mut self) -> Result<(), VmmError> {
        // TODO: Resume vCPUs via WHP
        Ok(())
    }

    fn step(&mut self) -> Result<(), VmmError> {
        // TODO: Single-step one vCPU exit
        Ok(())
    }

    fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError> {
        // TODO: Clone partition state via WHP
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
        // TODO: Create or destroy WHP vCPUs to match count
        Ok(())
    }

    fn adjust_balloon(&mut self, _target_mb: u32) -> Result<(), VmmError> {
        // TODO: Deflate/inflate virtio-balloon device descriptor
        Ok(())
    }

    fn skew_clock(&mut self, _offset_ns: i64) -> Result<(), VmmError> {
        // TODO: Adjust WHP virtual clock offset
        Ok(())
    }
}
