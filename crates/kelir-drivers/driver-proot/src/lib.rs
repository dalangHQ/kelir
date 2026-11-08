#![forbid(unsafe_code)]
// see: cans/01-architecture.md#Driver-backends
// see: cans/artifacts/diagrams/host-runtime.txt#driver-proot
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
pub struct ProotDriver {
    allow_emulated_fallback: bool,
    config: Option<VmConfig>,
    state: DriverState,
}

impl ProotDriver {
    pub fn new(allow_emulated_fallback: bool) -> Result<Self, VmmError> {
        if !allow_emulated_fallback {
            return Err(VmmError::Unsupported(
                "emulated fallback not allowed".to_string(),
            ));
        }
        Ok(Self {
            allow_emulated_fallback,
            config: None,
            state: DriverState::Unbooted,
        })
    }

    fn init_tracer(&mut self) -> Result<(), VmmError> {
        // TODO (L3): initialize proot ptrace tracing harness.
        Err(VmmError::Unsupported(
            "proot boot: ptrace tracing harness init (L3)".into(),
        ))
    }
}

#[async_trait]
impl VmmDriver for ProotDriver {
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities {
            backend: "proot",
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
        // TODO (L3): deterministic replay needs hypervisor clock control (kvm-clock/KVM_SET_CLOCK); a ptrace tracer cannot intercept VDSO clock reads.
        reject_emulated_deterministic_clock("proot", config.deterministic_clock)?;
        self.config = Some(config);
        self.init_tracer()
    }

    async fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError> {
        self.state.ensure_booted("proot")?;
        // TODO (L3): clone VM state via CoW fork in <15ms per cans/artifacts/code/vmm-driver-trait.rs.
        Err(VmmError::Unsupported(
            "proot fork: traced child fork (L3)".into(),
        ))
    }

    async fn checkpoint(&mut self, _label: &str) -> Result<SnapshotDescriptor, VmmError> {
        self.state.ensure_booted("proot")?;
        // TODO (L3): capture an atomic in-memory checkpoint of guest RAM and vCPU registers.
        Err(VmmError::Unsupported(
            "proot checkpoint: RAM + register capture (L3)".into(),
        ))
    }

    async fn map_shared_region(
        &mut self,
        _tag: &str,
        _size_bytes: usize,
    ) -> Result<SharedMemoryHandle, VmmError> {
        self.state.ensure_booted("proot")?;
        // TODO (L3): map an anonymous zero-copy host memory segment into guest address space.
        Err(VmmError::Unsupported(
            "proot map_shared_region: zero-copy segment map (L3)".into(),
        ))
    }

    async fn export_snapshot(&self) -> Result<Box<dyn AsyncRead + Send + Sync + Unpin>, VmmError> {
        self.state.ensure_booted("proot")?;
        // TODO (L3): serialize registers and compressed dirty pages into a portable stream.
        Err(VmmError::Unsupported(
            "proot export_snapshot: portable snapshot stream (L3)".into(),
        ))
    }

    async fn import_snapshot(
        &mut self,
        _reader: Box<dyn AsyncRead + Send + Sync + Unpin>,
    ) -> Result<(), VmmError> {
        self.state.ensure_booted("proot")?;
        // TODO (L3): hydrate microVM registers and memory pages from a portable snapshot stream.
        Err(VmmError::Unsupported(
            "proot import_snapshot: portable snapshot hydrate (L3)".into(),
        ))
    }

    async fn rollback(&mut self, _snapshot: &SnapshotDescriptor) -> Result<(), VmmError> {
        self.state.ensure_booted("proot")?;
        // TODO (L3): reset vCPU registers and revert memory tracking in <8ms.
        Err(VmmError::Unsupported(
            "proot rollback: child state reset (L3)".into(),
        ))
    }

    async fn suspend(
        &mut self,
        _reason: SuspendReason,
        _output_path: &Path,
    ) -> Result<(), VmmError> {
        self.state.ensure_booted("proot")?;
        // TODO (L3): flush dirty RAM pages to disk and drop host memory to 0 MB in <50ms.
        Err(VmmError::Unsupported(
            "proot suspend: flush dirty pages to disk (L3)".into(),
        ))
    }

    async fn resume(&mut self, _image_path: &Path) -> Result<(), VmmError> {
        self.state.ensure_booted("proot")?;
        // TODO (L3): restore a suspended VM image from storage in <50ms.
        Err(VmmError::Unsupported(
            "proot resume: restore suspended image (L3)".into(),
        ))
    }

    async fn skew_clock(&mut self, _offset: Duration) -> Result<(), VmmError> {
        self.state.ensure_booted("proot")?;
        // TODO (L3): offset the virtual hardware clock.
        Err(VmmError::Unsupported(
            "proot skew_clock: emulated clock offset (L3)".into(),
        ))
    }

    async fn adjust_balloon(&mut self, target_mb: u32) -> Result<BalloonConvergence, VmmError> {
        self.state.ensure_booted("proot")?;
        // TODO (L3): adjust the guest RAM ceiling via virtio-balloon; fork, snapshot, and heavy exec require prior convergence.
        let _ = target_mb;
        Err(VmmError::Unsupported(
            "proot adjust_balloon: emulated memory ceiling (L3)".into(),
        ))
    }

    // see: cans/10-attestation.md#Lifecycle-bridge
    async fn freeze_vcpus(&mut self) -> Result<(), VmmError> {
        self.state.ensure_booted("proot")?;
        // TODO (L3): halt vCPUs at the cycle boundary; guest RAM stays resident.
        Err(VmmError::Unsupported(
            "proot freeze_vcpus: vCPU-only halt (L3)".into(),
        ))
    }

    // see: cans/10-attestation.md#Lifecycle-bridge
    async fn unfreeze_vcpus(&mut self) -> Result<(), VmmError> {
        self.state.ensure_booted("proot")?;
        // TODO (L3): release halted vCPUs back to the scheduler.
        Err(VmmError::Unsupported(
            "proot unfreeze_vcpus: vCPU resume (L3)".into(),
        ))
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    async fn drain_fuse(&mut self, _timeout: Duration) -> Result<(), VmmError> {
        self.state.ensure_booted("proot")?;
        // TODO (L3): drain in-flight virtio-fs ops until quiescent or timeout.
        Err(VmmError::Unsupported(
            "proot drain_fuse: in-flight FUSE drain (L3)".into(),
        ))
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    async fn commit_broker_upper(&mut self) -> Result<(), VmmError> {
        self.state.ensure_booted("proot")?;
        // TODO (L3): replay the broker upper file and tombstones onto the base.
        Err(VmmError::Unsupported(
            "proot commit_broker_upper: broker upper commit (L3)".into(),
        ))
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    async fn discard_broker_upper(&mut self) -> Result<(), VmmError> {
        self.state.ensure_booted("proot")?;
        // TODO (L3): drop the broker upper file and tombstones; base untouched.
        Err(VmmError::Unsupported(
            "proot discard_broker_upper: broker upper discard (L3)".into(),
        ))
    }

    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    async fn step_clock_window(&mut self, _retired_instructions: u64) -> Result<(), VmmError> {
        self.state.ensure_booted("proot")?;
        // TODO (L3): advance KVM_SET_CLOCK / CNTVOFF_EL2 on the retired-instruction window.
        Err(VmmError::Unsupported(
            "proot step_clock_window: retired-instruction window clock step (L3)".into(),
        ))
    }

    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    fn register_pmu_sampler(&mut self, _sampler: Arc<dyn PmuSampler>) -> Result<(), VmmError> {
        // TODO (L3): arm host PMU counters and dispatch retired-instruction samples.
        Err(VmmError::Unsupported(
            "proot register_pmu_sampler: PMU sample callback registration (L3)".into(),
        ))
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn set_entropy_backend(&mut self, _backend: EntropyBackend) -> Result<(), VmmError> {
        self.state.ensure_booted("proot")?;
        // TODO (L3): swap virtio-rng for the seeded ChaCha20 stream under replay.
        Err(VmmError::Unsupported(
            "proot set_entropy_backend: virtio-rng backend swap (L3)".into(),
        ))
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn entropy_position(&self) -> Result<EntropyStreamPosition, VmmError> {
        self.state.ensure_booted("proot")?;
        // TODO (L3): report the live entropy stream offset.
        Err(VmmError::Unsupported(
            "proot entropy_position: stream position read (L3)".into(),
        ))
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn restore_entropy_position(
        &mut self,
        _position: EntropyStreamPosition,
    ) -> Result<(), VmmError> {
        self.state.ensure_booted("proot")?;
        // TODO (L3): rewind the entropy stream to the snapshot offset.
        Err(VmmError::Unsupported(
            "proot restore_entropy_position: stream position restore (L3)".into(),
        ))
    }

    async fn set_watchdog(
        &mut self,
        _timeout: Duration,
        _action: WatchdogAction,
    ) -> Result<(), VmmError> {
        self.state.ensure_booted("proot")?;
        // TODO (L3): arm the hardware watchdog; on timeout the driver executes the action.
        Err(VmmError::Unsupported(
            "proot set_watchdog: hardware watchdog arm (L3)".into(),
        ))
    }

    fn transport_connect(&self, _channel_id: u32) -> Result<Box<dyn Stream>, VmmError> {
        self.state.ensure_booted("proot")?;
        // TODO (L3): yield a raw duplex channel to the transport backend multiplexer.
        Err(VmmError::Unsupported(
            "proot transport_connect: duplex channel (L3)".into(),
        ))
    }

    async fn terminate(&mut self) -> Result<(), VmmError> {
        self.state.ensure_booted("proot")?;
        // TODO (L3): terminate the hypervisor instance and release memory allocations.
        Err(VmmError::Unsupported(
            "proot terminate: hypervisor teardown (L3)".into(),
        ))
    }
}
