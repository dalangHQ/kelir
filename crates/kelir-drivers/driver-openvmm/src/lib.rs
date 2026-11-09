#![forbid(unsafe_code)]
// see: cans/01-architecture.md#Driver-backends
// see: cans/artifacts/diagrams/host-runtime.txt#driver-openvmm
// TODO (L2): Windows named-pipe fallback arrives with transport_connect (no HV_SOCKET, per standing decision).
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::AsyncRead;

use kelir_interface::driver::{
    BalloonConvergence, DriverCapabilities, DriverState, EntropyBackend, EntropyStreamPosition,
    PmuSampler, SharedMemoryHandle, SnapshotDescriptor, Stream, SuspendReason, VmConfig, VmmDriver,
    VmmError, WatchdogAction, check_hw_lockfile,
};

#[allow(dead_code)]
pub struct OpenVmmDriver {
    partition_handle: usize,
    config: Option<VmConfig>,
    state: DriverState,
    entropy_backend: Option<EntropyBackend>,
    entropy_offset: u64,
}

impl OpenVmmDriver {
    pub fn new() -> Result<Self, VmmError> {
        Ok(Self {
            partition_handle: 0,
            config: None,
            state: DriverState::Unbooted,
            entropy_backend: None,
            entropy_offset: 0,
        })
    }

    // see: cans/01-architecture.md#Driver-backends
    pub fn partition_handle(&self) -> usize {
        self.partition_handle
    }

    fn create_partition(&mut self) -> Result<(), VmmError> {
        // TODO (L3): create the WHP partition via WHvCreatePartition and commit Local APIC state before the TSC deadline.
        Err(VmmError::Unsupported(
            "openvmm boot: WHvCreatePartition (L3)".into(),
        ))
    }
}

#[async_trait]
impl VmmDriver for OpenVmmDriver {
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities {
            backend: "openvmm",
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
        self.create_partition()
    }

    async fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError> {
        self.state.ensure_booted("openvmm")?;
        // TODO (L3): clone VM state via CoW fork in <15ms per cans/artifacts/code/vmm-driver-trait.rs.
        Err(VmmError::Unsupported(
            "openvmm fork: WHP partition clone (L3)".into(),
        ))
    }

    async fn checkpoint(&mut self, _label: &str) -> Result<SnapshotDescriptor, VmmError> {
        self.state.ensure_booted("openvmm")?;
        // TODO (L3): capture an atomic in-memory checkpoint of guest RAM and vCPU registers.
        Err(VmmError::Unsupported(
            "openvmm checkpoint: RAM + register capture (L3)".into(),
        ))
    }

    async fn map_shared_region(
        &mut self,
        _tag: &str,
        _size_bytes: usize,
    ) -> Result<SharedMemoryHandle, VmmError> {
        self.state.ensure_booted("openvmm")?;
        // TODO (L3): map an anonymous zero-copy host memory segment into guest address space.
        Err(VmmError::Unsupported(
            "openvmm map_shared_region: zero-copy segment map (L3)".into(),
        ))
    }

    async fn export_snapshot(&self) -> Result<Box<dyn AsyncRead + Send + Sync + Unpin>, VmmError> {
        self.state.ensure_booted("openvmm")?;
        // TODO (L3): serialize registers and compressed dirty pages into a portable stream.
        Err(VmmError::Unsupported(
            "openvmm export_snapshot: portable snapshot stream (L3)".into(),
        ))
    }

    async fn import_snapshot(
        &mut self,
        _reader: Box<dyn AsyncRead + Send + Sync + Unpin>,
    ) -> Result<(), VmmError> {
        self.state.ensure_booted("openvmm")?;
        // TODO (L3): hydrate microVM registers and memory pages from a portable snapshot stream.
        Err(VmmError::Unsupported(
            "openvmm import_snapshot: portable snapshot hydrate (L3)".into(),
        ))
    }

    async fn rollback(&mut self, _snapshot: &SnapshotDescriptor) -> Result<(), VmmError> {
        self.state.ensure_booted("openvmm")?;
        // TODO (L3): reset vCPU registers and revert memory tracking in <8ms.
        Err(VmmError::Unsupported(
            "openvmm rollback: register reset + memory revert (L3)".into(),
        ))
    }

    async fn suspend(
        &mut self,
        _reason: SuspendReason,
        _output_path: &Path,
    ) -> Result<(), VmmError> {
        self.state.ensure_booted("openvmm")?;
        // TODO (L3): flush dirty RAM pages to disk and drop host memory to 0 MB in <50ms.
        Err(VmmError::Unsupported(
            "openvmm suspend: flush dirty pages to disk (L3)".into(),
        ))
    }

    async fn resume(&mut self, _image_path: &Path) -> Result<(), VmmError> {
        self.state.ensure_booted("openvmm")?;
        // TODO (L3): restore a suspended VM image from storage in <50ms.
        Err(VmmError::Unsupported(
            "openvmm resume: restore suspended image (L3)".into(),
        ))
    }

    async fn skew_clock(&mut self, _offset: Duration) -> Result<(), VmmError> {
        self.state.ensure_booted("openvmm")?;
        // TODO (L3): offset the virtual hardware clock.
        Err(VmmError::Unsupported(
            "openvmm skew_clock: WHP clock offset (L3)".into(),
        ))
    }

    async fn adjust_balloon(&mut self, target_mb: u32) -> Result<BalloonConvergence, VmmError> {
        self.state.ensure_booted("openvmm")?;
        // TODO (L3): adjust the guest RAM ceiling via virtio-balloon; fork, snapshot, and heavy exec require prior convergence.
        let _ = target_mb;
        Err(VmmError::Unsupported(
            "openvmm adjust_balloon: virtio-balloon resize (L3)".into(),
        ))
    }

    // see: cans/10-attestation.md#Lifecycle-bridge
    async fn freeze_vcpus(&mut self) -> Result<(), VmmError> {
        self.state.ensure_booted("openvmm")?;
        // TODO (L3): halt vCPUs at the cycle boundary; guest RAM stays resident.
        Err(VmmError::Unsupported(
            "openvmm freeze_vcpus: vCPU-only halt (L3)".into(),
        ))
    }

    // see: cans/10-attestation.md#Lifecycle-bridge
    async fn unfreeze_vcpus(&mut self) -> Result<(), VmmError> {
        self.state.ensure_booted("openvmm")?;
        // TODO (L3): release halted vCPUs back to the scheduler.
        Err(VmmError::Unsupported(
            "openvmm unfreeze_vcpus: vCPU resume (L3)".into(),
        ))
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    async fn drain_fuse(&mut self, _timeout: Duration) -> Result<(), VmmError> {
        self.state.ensure_booted("openvmm")?;
        // TODO (L3): drain in-flight virtio-fs ops until quiescent or timeout.
        Err(VmmError::Unsupported(
            "openvmm drain_fuse: in-flight FUSE drain (L3)".into(),
        ))
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    async fn commit_broker_upper(&mut self) -> Result<(), VmmError> {
        self.state.ensure_booted("openvmm")?;
        // TODO (L3): replay the broker upper file and tombstones onto the base.
        Err(VmmError::Unsupported(
            "openvmm commit_broker_upper: broker upper commit (L3)".into(),
        ))
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    async fn discard_broker_upper(&mut self) -> Result<(), VmmError> {
        self.state.ensure_booted("openvmm")?;
        // TODO (L3): drop the broker upper file and tombstones; base untouched.
        Err(VmmError::Unsupported(
            "openvmm discard_broker_upper: broker upper discard (L3)".into(),
        ))
    }

    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    async fn step_clock_window(&mut self, _retired_instructions: u64) -> Result<(), VmmError> {
        self.state.ensure_booted("openvmm")?;
        // TODO (L3): advance KVM_SET_CLOCK / CNTVOFF_EL2 on the retired-instruction window.
        Err(VmmError::Unsupported(
            "openvmm step_clock_window: retired-instruction window clock step (L3)".into(),
        ))
    }

    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    fn register_pmu_sampler(&mut self, _sampler: Arc<dyn PmuSampler>) -> Result<(), VmmError> {
        // TODO (L3): arm host PMU counters and dispatch retired-instruction samples.
        Err(VmmError::Unsupported(
            "openvmm register_pmu_sampler: PMU sample callback registration (L3)".into(),
        ))
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn set_entropy_backend(&mut self, backend: EntropyBackend) -> Result<(), VmmError> {
        self.state.ensure_booted("openvmm")?;
        // TODO (L3): swap the virtio-rng device model for the seeded ChaCha20 stream and advance the offset as getrandom() bytes are drawn.
        self.entropy_backend = Some(backend);
        self.entropy_offset = 0;
        Ok(())
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn entropy_position(&self) -> Result<EntropyStreamPosition, VmmError> {
        self.state.ensure_booted("openvmm")?;
        match self.entropy_backend {
            Some(EntropyBackend::SeededChaCha20 { .. }) => Ok(EntropyStreamPosition {
                offset_bytes: self.entropy_offset,
            }),
            _ => Err(VmmError::Unsupported(
                "openvmm entropy_position: host entropy stream is not position-trackable (L3)"
                    .into(),
            )),
        }
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn restore_entropy_position(
        &mut self,
        position: EntropyStreamPosition,
    ) -> Result<(), VmmError> {
        self.state.ensure_booted("openvmm")?;
        match self.entropy_backend {
            Some(EntropyBackend::SeededChaCha20 { .. }) => {
                self.entropy_offset = position.offset_bytes;
                Ok(())
            }
            _ => Err(VmmError::Unsupported(
                "openvmm restore_entropy_position: host entropy stream is not position-trackable (L3)"
                    .into(),
            )),
        }
    }

    async fn set_watchdog(
        &mut self,
        _timeout: Duration,
        _action: WatchdogAction,
    ) -> Result<(), VmmError> {
        self.state.ensure_booted("openvmm")?;
        // TODO (L3): arm the hardware watchdog; on timeout the driver executes the action.
        Err(VmmError::Unsupported(
            "openvmm set_watchdog: hardware watchdog arm (L3)".into(),
        ))
    }

    fn transport_connect(&self, _channel_id: u32) -> Result<Box<dyn Stream>, VmmError> {
        self.state.ensure_booted("openvmm")?;
        // TODO (L3): yield a raw duplex channel to the transport backend multiplexer.
        Err(VmmError::Unsupported(
            "openvmm transport_connect: duplex channel (L3)".into(),
        ))
    }

    async fn terminate(&mut self) -> Result<(), VmmError> {
        self.state.ensure_booted("openvmm")?;
        // TODO (L3): terminate the hypervisor instance and release memory allocations.
        Err(VmmError::Unsupported(
            "openvmm terminate: hypervisor teardown (L3)".into(),
        ))
    }
}
