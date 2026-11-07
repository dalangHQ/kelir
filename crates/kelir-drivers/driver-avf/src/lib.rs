#![forbid(unsafe_code)]
// see: cans/01-architecture.md#Driver-backends
// see: cans/artifacts/diagrams/host-runtime.txt
use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::AsyncRead;

use kelir_interface::driver::{
    BalloonConvergence, SharedMemoryHandle, SnapshotDescriptor, Stream, SuspendReason, VmConfig,
    VmmDriver, VmmError, WatchdogAction,
};

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
pub struct AvfDriver {
    avf_fd: i32,
    config: Option<VmConfig>,
    state: DriverState,
}

impl AvfDriver {
    pub fn new() -> Result<Self, VmmError> {
        Ok(Self {
            avf_fd: -1,
            config: None,
            state: DriverState::Unbooted,
        })
    }

    pub fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities {
            backend: "avf",
            cold_boot_budget_ms: 60,
            rollback_budget_ms: 8,
            fork_budget_ms: 10,
            requires_hardware_vmm: true,
            supports_cow_fork: true,
        }
    }

    fn ensure_booted(&self) -> Result<(), VmmError> {
        match self.state {
            DriverState::Booted | DriverState::Paused => Ok(()),
            DriverState::Unbooted => Err(VmmError::InvalidState(
                "avf driver is not booted (L3)".into(),
            )),
        }
    }

    fn open_node(&mut self) -> Result<(), VmmError> {
        // TODO (L3): connect /dev/avf or the pKVM node.
        Err(VmmError::Unsupported("avf boot: /dev/avf open (L3)".into()))
    }
}

#[async_trait]
impl VmmDriver for AvfDriver {
    async fn boot(&mut self, config: VmConfig) -> Result<(), VmmError> {
        self.config = Some(config);
        self.open_node()
    }

    async fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError> {
        self.ensure_booted()?;
        // TODO (L3): clone VM state via CoW fork in <15ms per cans/artifacts/code/vmm-driver-trait.rs.
        Err(VmmError::Unsupported("avf fork: AVF VM clone (L3)".into()))
    }

    async fn checkpoint(&mut self, _label: &str) -> Result<SnapshotDescriptor, VmmError> {
        self.ensure_booted()?;
        // TODO (L3): capture an atomic in-memory checkpoint of guest RAM and vCPU registers.
        Err(VmmError::Unsupported(
            "avf checkpoint: RAM + register capture (L3)".into(),
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
            "avf map_shared_region: zero-copy segment map (L3)".into(),
        ))
    }

    async fn export_snapshot(&self) -> Result<Box<dyn AsyncRead + Send + Sync + Unpin>, VmmError> {
        self.ensure_booted()?;
        // TODO (L3): serialize registers and compressed dirty pages into a portable stream.
        Err(VmmError::Unsupported(
            "avf export_snapshot: portable snapshot stream (L3)".into(),
        ))
    }

    async fn import_snapshot(
        &mut self,
        _reader: Box<dyn AsyncRead + Send + Sync + Unpin>,
    ) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): hydrate microVM registers and memory pages from a portable snapshot stream.
        Err(VmmError::Unsupported(
            "avf import_snapshot: portable snapshot hydrate (L3)".into(),
        ))
    }

    async fn rollback(&mut self, _snapshot: &SnapshotDescriptor) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): reset vCPU registers and revert memory tracking in <8ms.
        Err(VmmError::Unsupported(
            "avf rollback: register reset + memory revert (L3)".into(),
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
            "avf suspend: flush dirty pages to disk (L3)".into(),
        ))
    }

    async fn resume(&mut self, _image_path: &Path) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): restore a suspended VM image from storage in <50ms.
        Err(VmmError::Unsupported(
            "avf resume: restore suspended image (L3)".into(),
        ))
    }

    async fn skew_clock(&mut self, _offset: Duration) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): offset the virtual hardware clock.
        Err(VmmError::Unsupported(
            "avf skew_clock: pKVM clock offset (L3)".into(),
        ))
    }

    async fn adjust_balloon(&mut self, target_mb: u32) -> Result<BalloonConvergence, VmmError> {
        self.ensure_booted()?;
        // TODO (L3): adjust the guest RAM ceiling via virtio-balloon; fork, snapshot, and heavy exec require prior convergence.
        let _ = target_mb;
        Err(VmmError::Unsupported(
            "avf adjust_balloon: virtio-balloon resize (L3)".into(),
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
            "avf set_watchdog: hardware watchdog arm (L3)".into(),
        ))
    }

    fn transport_connect(&self, _channel_id: u32) -> Result<Box<dyn Stream>, VmmError> {
        self.ensure_booted()?;
        // TODO (L3): yield a raw duplex channel to the transport backend multiplexer.
        Err(VmmError::Unsupported(
            "avf transport_connect: duplex channel (L3)".into(),
        ))
    }

    async fn terminate(&mut self) -> Result<(), VmmError> {
        self.ensure_booted()?;
        // TODO (L3): terminate the hypervisor instance and release memory allocations.
        Err(VmmError::Unsupported(
            "avf terminate: hypervisor teardown (L3)".into(),
        ))
    }
}
