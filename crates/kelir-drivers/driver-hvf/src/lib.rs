// see: cans/01-architecture.md#Host-memory-pressure-watcher
// see: cans/08-lifecycle.md#CoW-fork
// see: cans/artifacts/diagrams/host-runtime.txt
pub mod sys;

use kelir_interface::driver::{SnapshotDescriptor, VmConfig, VmmDriver, VmmError};

#[allow(dead_code)]
pub struct HvfDriver {
    vcpu_ids: Vec<sys::hv_vcpu_t>,
    memory_mapped_bytes: usize,
    config: Option<VmConfig>,
}

impl HvfDriver {
    pub fn new() -> Result<Self, VmmError> {
        Ok(Self {
            vcpu_ids: Vec::new(),
            memory_mapped_bytes: 0,
            config: None,
        })
    }

    #[allow(dead_code)]
    fn promote_chunked_stage2_pages(&self, _ipa: u64) -> Result<(), VmmError> {
        // see: cans/08-lifecycle.md#CoW-fork
        // TODO: Promote 2MB contiguous blocks via Mach vm_copy CoW
        Ok(())
    }
}

impl VmmDriver for HvfDriver {
    fn boot(&mut self, config: &VmConfig) -> Result<(), VmmError> {
        self.config = Some(config.clone());
        // TODO: Initialize Hypervisor.framework VM and vCPUs
        Ok(())
    }

    fn pause(&mut self) -> Result<(), VmmError> {
        // TODO: Pause vCPUs via hv_vcpu_run loop exit
        Ok(())
    }

    fn resume(&mut self) -> Result<(), VmmError> {
        // TODO: Resume vCPUs via hv_vcpu_run
        Ok(())
    }

    fn step(&mut self) -> Result<(), VmmError> {
        // TODO: Single-step one vCPU exit
        Ok(())
    }

    fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError> {
        // see: cans/08-lifecycle.md#CoW-fork
        // TODO: Mach copy-on-write + Stage-2 write protection via hv_vm_protect
        Ok(Box::new(Self::new()?))
    }

    fn rollback(&mut self, _snapshot: &SnapshotDescriptor) -> Result<(), VmmError> {
        // see: cans/08-lifecycle.md#Rollback
        // TODO: Reset Stage-2 translation table roots in <8ms
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
        // TODO: Create or destroy hv_vcpu_t handles to match count
        Ok(())
    }

    fn adjust_balloon(&mut self, _target_mb: u32) -> Result<(), VmmError> {
        // see: cans/01-architecture.md#Host-memory-pressure-watcher
        // TODO: Stage-2 hv_vm_unmap + mach_vm_deallocate purge
        Ok(())
    }

    fn skew_clock(&mut self, _offset_ns: i64) -> Result<(), VmmError> {
        // see: cans/08-lifecycle.md#Clock-skew
        // TODO: Adjust ARM64 CNTVOFF_EL2
        Ok(())
    }
}
