#![forbid(unsafe_code)]
// see: cans/01-architecture.md#Driver-backends
// see: cans/artifacts/diagrams/host-runtime.txt#driver-bwrap
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::AsyncRead;

use kelir_interface::driver::{
    BalloonConvergence, DriverCapabilities, DriverState, EntropyBackend, EntropyStreamPosition,
    PmuSampler, SharedMemoryHandle, SnapshotDescriptor, Stream, SuspendReason, VmConfig, VmmDriver,
    VmmError, WatchdogAction, check_hw_lockfile, reject_emulated_deterministic_clock,
};

#[allow(dead_code)]
pub struct BwrapDriver {
    allow_emulated_fallback: bool,
    child_pid: Option<libc::pid_t>,
    config: Option<VmConfig>,
    state: DriverState,
    entropy_backend: Option<EntropyBackend>,
    entropy_offset: u64,
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
            state: DriverState::Unbooted,
            entropy_backend: None,
            entropy_offset: 0,
        })
    }

    fn spawn_namespace(&mut self) -> Result<(), VmmError> {
        // TODO (L3): spawn bubblewrap/unshare user namespace via raw libc.
        Err(VmmError::Unsupported(
            "bwrap boot: bubblewrap namespace spawn (L3)".into(),
        ))
    }
}

#[async_trait]
impl VmmDriver for BwrapDriver {
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities {
            backend: "bwrap",
            cold_boot_budget_ms: 60,
            rollback_budget_ms: 8,
            fork_budget_ms: 10,
            requires_hardware_vmm: false,
            supports_cow_fork: false,
        }
    }

    async fn boot(&mut self, config: VmConfig) -> Result<(), VmmError> {
        // see: cans/15-operations.md#Developer-CLI
        check_hw_lockfile(&config.driver_policy, &self.capabilities())?;
        if !self.allow_emulated_fallback {
            return Err(VmmError::Unsupported(
                "emulated fallback not allowed".to_string(),
            ));
        }
        // TODO (L3): deterministic replay needs hypervisor clock control (kvm-clock/KVM_SET_CLOCK); a namespace wrapper cannot provide it.
        reject_emulated_deterministic_clock("bwrap", config.deterministic_clock)?;
        self.config = Some(config);
        self.spawn_namespace()
    }

    async fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError> {
        self.state.ensure_booted("bwrap")?;
        // TODO (L3): clone VM state via CoW fork in <15ms per cans/artifacts/code/vmm-driver-trait.rs.
        Err(VmmError::Unsupported(
            "bwrap fork: emulated namespace fork (L3)".into(),
        ))
    }

    async fn checkpoint(&mut self, _label: &str) -> Result<SnapshotDescriptor, VmmError> {
        self.state.ensure_booted("bwrap")?;
        // TODO (L3): capture an atomic in-memory checkpoint of guest RAM and vCPU registers.
        Err(VmmError::Unsupported(
            "bwrap checkpoint: RAM + register capture (L3)".into(),
        ))
    }

    async fn map_shared_region(
        &mut self,
        _tag: &str,
        _size_bytes: usize,
    ) -> Result<SharedMemoryHandle, VmmError> {
        self.state.ensure_booted("bwrap")?;
        // TODO (L3): map an anonymous zero-copy host memory segment into guest address space.
        Err(VmmError::Unsupported(
            "bwrap map_shared_region: zero-copy segment map (L3)".into(),
        ))
    }

    async fn export_snapshot(&self) -> Result<Box<dyn AsyncRead + Send + Sync + Unpin>, VmmError> {
        self.state.ensure_booted("bwrap")?;
        // TODO (L3): serialize registers and compressed dirty pages into a portable stream.
        Err(VmmError::Unsupported(
            "bwrap export_snapshot: portable snapshot stream (L3)".into(),
        ))
    }

    async fn import_snapshot(
        &mut self,
        _reader: Box<dyn AsyncRead + Send + Sync + Unpin>,
    ) -> Result<(), VmmError> {
        self.state.ensure_booted("bwrap")?;
        // TODO (L3): hydrate microVM registers and memory pages from a portable snapshot stream.
        Err(VmmError::Unsupported(
            "bwrap import_snapshot: portable snapshot hydrate (L3)".into(),
        ))
    }

    async fn rollback(&mut self, _snapshot: &SnapshotDescriptor) -> Result<(), VmmError> {
        self.state.ensure_booted("bwrap")?;
        // TODO (L3): reset vCPU registers and revert memory tracking in <8ms.
        Err(VmmError::Unsupported(
            "bwrap rollback: child state reset (L3)".into(),
        ))
    }

    async fn suspend(
        &mut self,
        _reason: SuspendReason,
        _output_path: &Path,
    ) -> Result<(), VmmError> {
        self.state.ensure_booted("bwrap")?;
        // TODO (L3): flush dirty RAM pages to disk and drop host memory to 0 MB in <50ms.
        Err(VmmError::Unsupported(
            "bwrap suspend: flush dirty pages to disk (L3)".into(),
        ))
    }

    async fn resume(&mut self, _image_path: &Path) -> Result<(), VmmError> {
        self.state.ensure_booted("bwrap")?;
        // TODO (L3): restore a suspended VM image from storage in <50ms.
        Err(VmmError::Unsupported(
            "bwrap resume: restore suspended image (L3)".into(),
        ))
    }

    async fn skew_clock(&mut self, _offset: Duration) -> Result<(), VmmError> {
        self.state.ensure_booted("bwrap")?;
        // TODO (L3): offset the virtual hardware clock.
        Err(VmmError::Unsupported(
            "bwrap skew_clock: emulated clock offset (L3)".into(),
        ))
    }

    async fn adjust_balloon(&mut self, target_mb: u32) -> Result<BalloonConvergence, VmmError> {
        self.state.ensure_booted("bwrap")?;
        // TODO (L3): adjust the guest RAM ceiling via virtio-balloon; fork, snapshot, and heavy exec require prior convergence.
        let _ = target_mb;
        Err(VmmError::Unsupported(
            "bwrap adjust_balloon: emulated memory ceiling (L3)".into(),
        ))
    }

    // see: cans/10-attestation.md#Lifecycle-bridge
    async fn freeze_vcpus(&mut self) -> Result<(), VmmError> {
        self.state.ensure_booted("bwrap")?;
        // TODO (L3): halt vCPUs at the cycle boundary; guest RAM stays resident.
        Err(VmmError::Unsupported(
            "bwrap freeze_vcpus: vCPU-only halt (L3)".into(),
        ))
    }

    // see: cans/10-attestation.md#Lifecycle-bridge
    async fn unfreeze_vcpus(&mut self) -> Result<(), VmmError> {
        self.state.ensure_booted("bwrap")?;
        // TODO (L3): release halted vCPUs back to the scheduler.
        Err(VmmError::Unsupported(
            "bwrap unfreeze_vcpus: vCPU resume (L3)".into(),
        ))
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    async fn drain_fuse(&mut self, _timeout: Duration) -> Result<(), VmmError> {
        self.state.ensure_booted("bwrap")?;
        // TODO (L3): drain in-flight virtio-fs ops until quiescent or timeout.
        Err(VmmError::Unsupported(
            "bwrap drain_fuse: in-flight FUSE drain (L3)".into(),
        ))
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    async fn commit_broker_upper(&mut self) -> Result<(), VmmError> {
        self.state.ensure_booted("bwrap")?;
        // TODO (L3): replay the broker upper file and tombstones onto the base.
        Err(VmmError::Unsupported(
            "bwrap commit_broker_upper: broker upper commit (L3)".into(),
        ))
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    async fn discard_broker_upper(&mut self) -> Result<(), VmmError> {
        self.state.ensure_booted("bwrap")?;
        // TODO (L3): drop the broker upper file and tombstones; base untouched.
        Err(VmmError::Unsupported(
            "bwrap discard_broker_upper: broker upper discard (L3)".into(),
        ))
    }

    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    async fn step_clock_window(&mut self, _retired_instructions: u64) -> Result<(), VmmError> {
        self.state.ensure_booted("bwrap")?;
        // TODO (L3): advance KVM_SET_CLOCK / CNTVOFF_EL2 on the retired-instruction window.
        Err(VmmError::Unsupported(
            "bwrap step_clock_window: retired-instruction window clock step (L3)".into(),
        ))
    }

    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    fn register_pmu_sampler(&mut self, _sampler: Arc<dyn PmuSampler>) -> Result<(), VmmError> {
        // TODO (L3): arm host PMU counters and dispatch retired-instruction samples.
        Err(VmmError::Unsupported(
            "bwrap register_pmu_sampler: PMU sample callback registration (L3)".into(),
        ))
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn set_entropy_backend(&mut self, backend: EntropyBackend) -> Result<(), VmmError> {
        self.state.ensure_booted("bwrap")?;
        // TODO (L3): swap the workload entropy source for the seeded ChaCha20 stream and advance the offset as getrandom() bytes are drawn.
        self.entropy_backend = Some(backend);
        self.entropy_offset = 0;
        Ok(())
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn entropy_position(&self) -> Result<EntropyStreamPosition, VmmError> {
        self.state.ensure_booted("bwrap")?;
        // TODO (L3): report the live seeded-stream offset; host entropy is not position-trackable.
        match self.entropy_backend {
            Some(EntropyBackend::SeededChaCha20 { .. }) => Ok(EntropyStreamPosition {
                offset_bytes: self.entropy_offset,
            }),
            _ => Err(VmmError::Unsupported(
                "bwrap entropy_position: host entropy stream is not position-trackable (L3)".into(),
            )),
        }
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn restore_entropy_position(
        &mut self,
        position: EntropyStreamPosition,
    ) -> Result<(), VmmError> {
        self.state.ensure_booted("bwrap")?;
        // TODO (L3): rewind the seeded stream to the restored offset before the next draw.
        match self.entropy_backend {
            Some(EntropyBackend::SeededChaCha20 { .. }) => {
                self.entropy_offset = position.offset_bytes;
                Ok(())
            }
            _ => Err(VmmError::Unsupported(
                "bwrap restore_entropy_position: host entropy stream is not position-trackable (L3)"
                    .into(),
            )),
        }
    }

    async fn set_watchdog(
        &mut self,
        _timeout: Duration,
        _action: WatchdogAction,
    ) -> Result<(), VmmError> {
        self.state.ensure_booted("bwrap")?;
        // TODO (L3): arm the hardware watchdog; on timeout the driver executes the action.
        Err(VmmError::Unsupported(
            "bwrap set_watchdog: hardware watchdog arm (L3)".into(),
        ))
    }

    fn transport_connect(&self, _channel_id: u32) -> Result<Box<dyn Stream>, VmmError> {
        self.state.ensure_booted("bwrap")?;
        // TODO (L3): yield a raw duplex channel to the transport backend multiplexer.
        Err(VmmError::Unsupported(
            "bwrap transport_connect: duplex channel (L3)".into(),
        ))
    }

    async fn terminate(&mut self) -> Result<(), VmmError> {
        self.state.ensure_booted("bwrap")?;
        // TODO (L3): terminate the hypervisor instance and release memory allocations.
        Err(VmmError::Unsupported(
            "bwrap terminate: hypervisor teardown (L3)".into(),
        ))
    }
}
