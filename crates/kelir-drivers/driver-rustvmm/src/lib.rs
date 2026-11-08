#![forbid(unsafe_code)]
// see: cans/01-architecture.md#Driver-backends
// see: cans/08-lifecycle.md#CoW-fork
// see: cans/artifacts/diagrams/host-runtime.txt#driver-rustvmm
// TODO (L3): tests/test_cow_fork.rs + test_rollback.rs per cans/artifacts/config/project-structure.txt#kelir-drivers.
use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::AsyncRead;

use kelir_interface::driver::{
    BalloonConvergence, DriverCapabilities, DriverState, SharedMemoryHandle, SnapshotDescriptor,
    Stream, SuspendReason, VmConfig, VmmDriver, VmmError, WatchdogAction, check_hw_lockfile,
};

#[allow(dead_code)]
pub struct RustVmmDriver {
    #[cfg(target_os = "linux")]
    kvm_fd: Option<kvm_ioctls::Kvm>,
    #[cfg(target_os = "linux")]
    vm_fd: Option<kvm_ioctls::VmFd>,
    #[cfg(target_os = "linux")]
    vcpu_fds: Vec<kvm_ioctls::VcpuFd>,
    #[cfg(target_os = "linux")]
    uffd: Option<userfaultfd::Uffd>,
    config: Option<VmConfig>,
    state: DriverState,
}

impl RustVmmDriver {
    pub fn new() -> Result<Self, VmmError> {
        #[cfg(not(target_os = "linux"))]
        return Err(VmmError::Unsupported(
            "driver-rustvmm requires Linux".into(),
        ));
        #[cfg(target_os = "linux")]
        Ok(Self {
            kvm_fd: None,
            vm_fd: None,
            vcpu_fds: Vec::new(),
            uffd: None,
            config: None,
            state: DriverState::Unbooted,
        })
    }

    fn init_backend(&mut self) -> Result<(), VmmError> {
        // TODO (L3): initialize KVM, VM, memory slots, and vCPUs via KVM_CREATE_VM.
        Err(VmmError::Unsupported(
            "rustvmm boot: KVM_CREATE_VM init (L3)".into(),
        ))
    }

    #[cfg(target_os = "linux")]
    #[allow(dead_code)]
    fn filter_synthetic_cpuid(&self, _vcpu: &kvm_ioctls::VcpuFd) -> Result<(), VmmError> {
        // see: cans/01-architecture.md#Driver-backends
        // TODO (L3): enforce invariant TSC and Leaf 0x1 TSC-deadline.
        Err(VmmError::Unsupported(
            "rustvmm cpuid filter: synthetic CPUID enforcement (L3)".into(),
        ))
    }
}

#[async_trait]
impl VmmDriver for RustVmmDriver {
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities {
            backend: "rustvmm",
            cold_boot_budget_ms: 60,
            rollback_budget_ms: 8,
            fork_budget_ms: 10,
            requires_hardware_vmm: true,
            supports_cow_fork: true,
        }
    }

    async fn boot(&mut self, config: VmConfig) -> Result<(), VmmError> {
        // see: cans/15-operations.md#Developer-CLI
        check_hw_lockfile(&config.driver_policy, &self.capabilities())?;
        self.config = Some(config);
        self.init_backend()
    }

    async fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError> {
        self.state.ensure_booted("rustvmm")?;
        // see: cans/08-lifecycle.md#CoW-fork
        // TODO (L3): clone VM state via CoW fork in <15ms per cans/artifacts/code/vmm-driver-trait.rs.
        Err(VmmError::Unsupported(
            "rustvmm fork: userfaultfd CoW fork (L3)".into(),
        ))
    }

    async fn checkpoint(&mut self, _label: &str) -> Result<SnapshotDescriptor, VmmError> {
        self.state.ensure_booted("rustvmm")?;
        // TODO (L3): capture an atomic in-memory checkpoint of guest RAM and vCPU registers.
        Err(VmmError::Unsupported(
            "rustvmm checkpoint: RAM + register capture (L3)".into(),
        ))
    }

    async fn map_shared_region(
        &mut self,
        _tag: &str,
        _size_bytes: usize,
    ) -> Result<SharedMemoryHandle, VmmError> {
        self.state.ensure_booted("rustvmm")?;
        // TODO (L3): map an anonymous zero-copy host memory segment into guest address space.
        Err(VmmError::Unsupported(
            "rustvmm map_shared_region: zero-copy segment map (L3)".into(),
        ))
    }

    async fn export_snapshot(&self) -> Result<Box<dyn AsyncRead + Send + Sync + Unpin>, VmmError> {
        self.state.ensure_booted("rustvmm")?;
        // TODO (L3): serialize registers and compressed dirty pages into a portable stream.
        Err(VmmError::Unsupported(
            "rustvmm export_snapshot: portable snapshot stream (L3)".into(),
        ))
    }

    async fn import_snapshot(
        &mut self,
        _reader: Box<dyn AsyncRead + Send + Sync + Unpin>,
    ) -> Result<(), VmmError> {
        self.state.ensure_booted("rustvmm")?;
        // TODO (L3): hydrate microVM registers and memory pages from a portable snapshot stream.
        Err(VmmError::Unsupported(
            "rustvmm import_snapshot: portable snapshot hydrate (L3)".into(),
        ))
    }

    async fn rollback(&mut self, _snapshot: &SnapshotDescriptor) -> Result<(), VmmError> {
        self.state.ensure_booted("rustvmm")?;
        // see: cans/08-lifecycle.md#Rollback
        // TODO (L3): reset vCPU registers and revert memory tracking in <8ms.
        Err(VmmError::Unsupported(
            "rustvmm rollback: register reset + memory revert (L3)".into(),
        ))
    }

    async fn suspend(
        &mut self,
        _reason: SuspendReason,
        _output_path: &Path,
    ) -> Result<(), VmmError> {
        self.state.ensure_booted("rustvmm")?;
        // TODO (L3): flush dirty RAM pages to disk and drop host memory to 0 MB in <50ms.
        Err(VmmError::Unsupported(
            "rustvmm suspend: flush dirty pages to disk (L3)".into(),
        ))
    }

    async fn resume(&mut self, _image_path: &Path) -> Result<(), VmmError> {
        self.state.ensure_booted("rustvmm")?;
        // TODO (L3): restore a suspended VM image from storage in <50ms.
        Err(VmmError::Unsupported(
            "rustvmm resume: restore suspended image (L3)".into(),
        ))
    }

    async fn skew_clock(&mut self, _offset: Duration) -> Result<(), VmmError> {
        self.state.ensure_booted("rustvmm")?;
        // see: cans/08-lifecycle.md#Clock-skew
        // TODO (L3): offset the virtual hardware clock.
        Err(VmmError::Unsupported(
            "rustvmm skew_clock: KVM_SET_CLOCK offset (L3)".into(),
        ))
    }

    async fn adjust_balloon(&mut self, target_mb: u32) -> Result<BalloonConvergence, VmmError> {
        self.state.ensure_booted("rustvmm")?;
        // TODO (L3): adjust the guest RAM ceiling via virtio-balloon; fork, snapshot, and heavy exec require prior convergence.
        let _ = target_mb;
        Err(VmmError::Unsupported(
            "rustvmm adjust_balloon: virtio-balloon resize (L3)".into(),
        ))
    }

    async fn set_watchdog(
        &mut self,
        _timeout: Duration,
        _action: WatchdogAction,
    ) -> Result<(), VmmError> {
        self.state.ensure_booted("rustvmm")?;
        // TODO (L3): arm the hardware watchdog; on timeout the driver executes the action.
        Err(VmmError::Unsupported(
            "rustvmm set_watchdog: hardware watchdog arm (L3)".into(),
        ))
    }

    fn transport_connect(&self, _channel_id: u32) -> Result<Box<dyn Stream>, VmmError> {
        self.state.ensure_booted("rustvmm")?;
        // TODO (L3): yield a raw duplex channel to the transport backend multiplexer.
        Err(VmmError::Unsupported(
            "rustvmm transport_connect: duplex channel (L3)".into(),
        ))
    }

    async fn terminate(&mut self) -> Result<(), VmmError> {
        self.state.ensure_booted("rustvmm")?;
        // TODO (L3): terminate the hypervisor instance and release memory allocations.
        Err(VmmError::Unsupported(
            "rustvmm terminate: hypervisor teardown (L3)".into(),
        ))
    }
}
