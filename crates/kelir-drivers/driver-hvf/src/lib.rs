// see: cans/01-architecture.md#Host-memory-pressure-watcher
// see: cans/08-lifecycle.md#CoW-fork
// see: cans/artifacts/diagrams/host-runtime.txt
pub mod sys;

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::AsyncRead;

use kelir_interface::driver::{
    BalloonConvergence, DriverCapabilities, DriverState, SharedMemoryHandle, SnapshotDescriptor,
    Stream, SuspendReason, VmConfig, VmmDriver, VmmError, WatchdogAction,
};

#[allow(dead_code)]
pub struct HvfDriver {
    vcpu_ids: Vec<sys::hv_vcpu_t>,
    memory_mapped_bytes: usize,
    config: Option<VmConfig>,
    state: DriverState,
}

impl HvfDriver {
    pub fn new() -> Result<Self, VmmError> {
        Ok(Self {
            vcpu_ids: Vec::new(),
            memory_mapped_bytes: 0,
            config: None,
            state: DriverState::Unbooted,
        })
    }

    pub fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities {
            backend: "hvf",
            cold_boot_budget_ms: 60,
            rollback_budget_ms: 8,
            fork_budget_ms: 10,
            requires_hardware_vmm: true,
            supports_cow_fork: true,
        }
    }

    fn ensure_booted(&self) -> Result<(), VmmError> {
        self.state.ensure_booted("hvf")
    }

    fn create_vm(&mut self) -> Result<(), VmmError> {
        // TODO (L3): call sys::hv_vm_create(0) inside an unsafe block, then sys::hv_vcpu_create per vCPU.
        Err(VmmError::Unsupported(
            "hvf boot: hv_vm_create + hv_vcpu_create (L3)".into(),
        ))
    }

    #[allow(dead_code)]
    fn run_vcpu(&mut self) -> Result<(), VmmError> {
        // TODO (L3): call sys::hv_vcpu_run on each vcpu handle inside an unsafe block.
        Err(VmmError::Unsupported("hvf run: hv_vcpu_run (L3)".into()))
    }

    fn protect_fork(&self) -> Result<(), VmmError> {
        // TODO (L3): Mach copy-on-write + Stage-2 write protection via sys::hv_vm_protect.
        Err(VmmError::Unsupported(
            "hvf fork: hv_vm_protect CoW fork (L3)".into(),
        ))
    }

    #[allow(dead_code)]
    fn promote_chunked_stage2_pages(&self, _ipa: u64) -> Result<(), VmmError> {
        // see: cans/08-lifecycle.md#CoW-fork
        // TODO (L3): promote 2MB contiguous blocks via Mach vm_copy CoW.
        Err(VmmError::Unsupported(
            "hvf stage-2 promotion: Mach vm_copy CoW (L3)".into(),
        ))
    }
}

#[async_trait]
impl VmmDriver for HvfDriver {
    async fn boot(&mut self, config: VmConfig) -> Result<(), VmmError> {
        self.config = Some(config);
        self.create_vm()
    }

    async fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError> {
        self.ensure_booted()?;
        self.protect_fork()?;
        // TODO (L3): clone VM state via CoW fork in <15ms per cans/artifacts/code/vmm-driver-trait.rs.
        Err(VmmError::Unsupported(
            "hvf fork: CoW child driver (L3)".into(),
        ))
    }

    async fn checkpoint(&mut self, _label: &str) -> Result<SnapshotDescriptor, VmmError> {
        self.ensure_booted()?;
        // TODO (L3): capture an atomic in-memory checkpoint of guest RAM and vCPU registers.
        Err(VmmError::Unsupported(
            "hvf checkpoint: RAM + register capture (L3)".into(),
        ))
    }

    async fn map_shared_region(
        &mut self,
        _tag: &str,
        _size_bytes: usize,
    ) -> Result<SharedMemoryHandle, VmmError> {
        self.ensure_booted()?;
        // TODO (L3): map an anonymous zero-copy host memory segment into guest address space.
        Err(VmmError::Unsupported(
            "hvf map_shared_region: zero-copy segment map (L3)".into(),
        ))
    }

    async fn export_snapshot(&self) -> Result<Box<dyn AsyncRead + Send + Sync + Unpin>, VmmError> {
        self.ensure_booted()?;
        // TODO (L3): serialize registers and compressed dirty pages into a portable stream.
        Err(VmmError::Unsupported(
            "hvf export_snapshot: portable snapshot stream (L3)".into(),
        ))
    }

    async fn import_snapshot(
        &mut self,
        _reader: Box<dyn AsyncRead + Send + Sync + Unpin>,
    ) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): hydrate microVM registers and memory pages from a portable snapshot stream.
        Err(VmmError::Unsupported(
            "hvf import_snapshot: portable snapshot hydrate (L3)".into(),
        ))
    }

    async fn rollback(&mut self, _snapshot: &SnapshotDescriptor) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // see: cans/08-lifecycle.md#Rollback
        // TODO (L3): reset vCPU registers and revert memory tracking in <8ms.
        Err(VmmError::Unsupported(
            "hvf rollback: Stage-2 root reset (L3)".into(),
        ))
    }

    async fn suspend(
        &mut self,
        _reason: SuspendReason,
        _output_path: &Path,
    ) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): flush dirty RAM pages to disk and drop host memory to 0 MB in <50ms.
        Err(VmmError::Unsupported(
            "hvf suspend: flush dirty pages to disk (L3)".into(),
        ))
    }

    async fn resume(&mut self, _image_path: &Path) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): restore a suspended VM image from storage in <50ms.
        Err(VmmError::Unsupported(
            "hvf resume: restore suspended image (L3)".into(),
        ))
    }

    async fn skew_clock(&mut self, _offset: Duration) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // see: cans/08-lifecycle.md#Clock-skew
        // TODO (L3): offset the virtual hardware clock.
        Err(VmmError::Unsupported(
            "hvf skew_clock: CNTVOFF_EL2 offset (L3)".into(),
        ))
    }

    async fn adjust_balloon(&mut self, target_mb: u32) -> Result<BalloonConvergence, VmmError> {
        self.ensure_booted()?;
        // see: cans/01-architecture.md#Host-memory-pressure-watcher
        // TODO (L3): adjust the guest RAM ceiling via virtio-balloon; fork, snapshot, and heavy exec require prior convergence.
        let _ = target_mb;
        Err(VmmError::Unsupported(
            "hvf adjust_balloon: hv_vm_unmap purge (L3)".into(),
        ))
    }

    async fn set_watchdog(
        &mut self,
        _timeout: Duration,
        _action: WatchdogAction,
    ) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): arm the hardware watchdog; on timeout the driver executes the action.
        Err(VmmError::Unsupported(
            "hvf set_watchdog: hardware watchdog arm (L3)".into(),
        ))
    }

    fn transport_connect(&self, _channel_id: u32) -> Result<Box<dyn Stream>, VmmError> {
        self.ensure_booted()?;
        // TODO (L3): yield a raw duplex channel to the transport backend multiplexer.
        Err(VmmError::Unsupported(
            "hvf transport_connect: duplex channel (L3)".into(),
        ))
    }

    async fn terminate(&mut self) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): terminate the hypervisor instance and release memory allocations.
        Err(VmmError::Unsupported(
            "hvf terminate: hypervisor teardown (L3)".into(),
        ))
    }
}
