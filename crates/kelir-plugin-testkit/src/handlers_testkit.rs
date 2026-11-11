// see: cans/02-plugin-model.md#Plugin-testkit
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use kelir_interface::driver_interface::{
    BalloonConvergence, DriverCapabilities, DriverError, EntropyBackend, EntropyStreamPosition,
    PmuSampler, SharedMemoryHandle, SnapshotDescriptor, SuspendReason, VmConfig, VmmDriver,
    WatchdogAction, default_vm_config,
};
use kelir_interface::plugin_model::{HandlerCtx, LifecycleCtx, VetoList};
use kelir_interface::protocol_wire::TrapEvent;
use tokio::io::AsyncReadExt;

pub fn handler_ctx(session_id: [u8; 16], pid: u32) -> HandlerCtx<(), ()> {
    HandlerCtx::restricted(session_id, pid)
}

pub fn handler_ctx_for_trap(session_id: [u8; 16], event: &TrapEvent) -> HandlerCtx<(), ()> {
    HandlerCtx::restricted(session_id, event.pid).with_trap(event.clone())
}

// see: cans/02-plugin-model.md#Plugin-testkit
// see: cans/08-lifecycle.md#Hypervisor-primitives
#[derive(Clone)]
enum FixturePhase {
    Created,
    Running,
    Suspended,
    Terminated,
}

// see: cans/02-plugin-model.md#Plugin-testkit
// see: cans/08-lifecycle.md#Hypervisor-primitives
#[derive(Clone)]
struct FixtureLedger {
    snapshots: Vec<SnapshotDescriptor>,
    snapshot_seq: u64,
    balloon_mb: Option<u32>,
    clock_offset_ns: u64,
    retired_instructions: u64,
    entropy_offset_bytes: u64,
    broker_upper_committed: u64,
    broker_upper_discarded: u64,
}

impl FixtureLedger {
    fn new() -> Self {
        Self {
            snapshots: Vec::new(),
            snapshot_seq: 0,
            balloon_mb: None,
            clock_offset_ns: 0,
            retired_instructions: 0,
            entropy_offset_bytes: 0,
            broker_upper_committed: 0,
            broker_upper_discarded: 0,
        }
    }

    // see: cans/02-plugin-model.md#Plugin-testkit
    fn to_image(&self) -> String {
        let snapshots: Vec<serde_json::Value> = self
            .snapshots
            .iter()
            .map(|descriptor| {
                serde_json::json!({
                    "id": descriptor.id,
                    "dirty_bytes": descriptor.dirty_bytes,
                    "created_at_ns": descriptor.created_at_ns,
                    "parent_digest": descriptor.parent_digest,
                })
            })
            .collect();
        serde_json::to_string(&serde_json::json!({
            "backend": "fixture",
            "snapshots": snapshots,
            "snapshot_seq": self.snapshot_seq,
            "balloon_mb": self.balloon_mb,
            "clock_offset_ns": self.clock_offset_ns,
            "retired_instructions": self.retired_instructions,
            "entropy_offset_bytes": self.entropy_offset_bytes,
            "broker_upper_committed": self.broker_upper_committed,
            "broker_upper_discarded": self.broker_upper_discarded,
        }))
        .unwrap_or_default()
    }

    // see: cans/02-plugin-model.md#Plugin-testkit
    fn from_image(text: &str) -> Result<Self, DriverError> {
        let value: serde_json::Value = serde_json::from_str(text).map_err(|e| {
            DriverError::InvalidState(format!("fixture snapshot image is not valid JSON: {e}"))
        })?;
        let mut ledger = Self::new();
        for snapshot in value
            .get("snapshots")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            ledger.snapshots.push(SnapshotDescriptor {
                id: snapshot
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                dirty_bytes: snapshot
                    .get("dirty_bytes")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or_default(),
                created_at_ns: snapshot
                    .get("created_at_ns")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or_default(),
                parent_digest: snapshot
                    .get("parent_digest")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string),
                guest_clock: None,
            });
        }
        let counter = |key: &str| {
            value
                .get(key)
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default()
        };
        ledger.snapshot_seq = counter("snapshot_seq");
        ledger.clock_offset_ns = counter("clock_offset_ns");
        ledger.retired_instructions = counter("retired_instructions");
        ledger.entropy_offset_bytes = counter("entropy_offset_bytes");
        ledger.broker_upper_committed = counter("broker_upper_committed");
        ledger.broker_upper_discarded = counter("broker_upper_discarded");
        ledger.balloon_mb = value
            .get("balloon_mb")
            .and_then(serde_json::Value::as_u64)
            .and_then(|mb| u32::try_from(mb).ok());
        Ok(ledger)
    }
}

// see: cans/02-plugin-model.md#Plugin-testkit
// see: cans/08-lifecycle.md#Hypervisor-primitives
#[derive(Clone)]
pub struct FixtureVmmDriver {
    phase: FixturePhase,
    armed_watchdog: Option<(Duration, WatchdogAction)>,
    entropy_backend: Option<EntropyBackend>,
    frozen_vcpus: bool,
    last_drain_timeout: Option<Duration>,
    pmu_samplers: Vec<Arc<dyn PmuSampler>>,
    ledger: FixtureLedger,
}

impl FixtureVmmDriver {
    pub fn new() -> Self {
        Self {
            phase: FixturePhase::Created,
            armed_watchdog: None,
            entropy_backend: None,
            frozen_vcpus: false,
            last_drain_timeout: None,
            pmu_samplers: Vec::new(),
            ledger: FixtureLedger::new(),
        }
    }

    pub fn armed_watchdog(&self) -> Option<(Duration, WatchdogAction)> {
        self.armed_watchdog.clone()
    }

    fn require_running(&self) -> Result<(), DriverError> {
        match self.phase {
            FixturePhase::Running => Ok(()),
            FixturePhase::Created => Err(DriverError::InvalidState(
                "fixture driver has not booted".to_string(),
            )),
            FixturePhase::Suspended => Err(DriverError::InvalidState(
                "fixture driver is suspended".to_string(),
            )),
            FixturePhase::Terminated => Err(DriverError::InvalidState(
                "fixture driver has terminated".to_string(),
            )),
        }
    }
}

impl Default for FixtureVmmDriver {
    fn default() -> Self {
        Self::new()
    }
}

// see: cans/02-plugin-model.md#Plugin-testkit
// see: cans/08-lifecycle.md#Hypervisor-primitives
struct FixtureSnapshotReader {
    data: Vec<u8>,
    pos: usize,
}

impl tokio::io::AsyncRead for FixtureSnapshotReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let remaining = self.data.len() - self.pos;
        if remaining == 0 {
            return std::task::Poll::Ready(Ok(()));
        }
        let end = self.pos + remaining.min(buf.remaining());
        let chunk = &self.data[self.pos..end];
        buf.put_slice(chunk);
        self.pos = end;
        std::task::Poll::Ready(Ok(()))
    }
}

#[async_trait]
impl VmmDriver for FixtureVmmDriver {
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities {
            backend: "fixture",
            cold_boot_budget_ms: 60,
            rollback_budget_ms: 8,
            fork_budget_ms: 10,
            requires_hardware_vmm: false,
            supports_cow_fork: true,
        }
    }

    async fn boot(&mut self, _config: VmConfig) -> Result<(), DriverError> {
        match self.phase {
            FixturePhase::Created => {
                self.phase = FixturePhase::Running;
                Ok(())
            }
            _ => Err(DriverError::InvalidState(
                "fixture driver boots only from the created phase".to_string(),
            )),
        }
    }

    // see: cans/01-architecture.md#Driver-backends
    fn is_emulated(&self) -> Result<bool, DriverError> {
        Ok(true)
    }

    // see: cans/08-lifecycle.md#CoW-fork
    async fn fork(&self) -> Result<Box<dyn VmmDriver>, DriverError> {
        self.require_running()?;
        Ok(Box::new(<FixtureVmmDriver as Clone>::clone(self)))
    }

    // see: cans/08-lifecycle.md#Rollback
    async fn checkpoint(&mut self, label: &str) -> Result<SnapshotDescriptor, DriverError> {
        self.require_running()?;
        self.ledger.snapshot_seq += 1;
        let descriptor = SnapshotDescriptor {
            id: format!("{label}-{}", self.ledger.snapshot_seq),
            dirty_bytes: 0,
            created_at_ns: self.ledger.snapshot_seq * 1_000_000,
            parent_digest: self.ledger.snapshots.last().map(|parent| parent.id.clone()),
            guest_clock: None,
        };
        self.ledger.snapshots.push(descriptor.clone());
        Ok(descriptor)
    }

    async fn map_shared_region(
        &mut self,
        _tag: &str,
        _size_bytes: usize,
    ) -> Result<SharedMemoryHandle, DriverError> {
        Err(DriverError::Unsupported(
            "map_shared_region needs a real memfd; a fixture cannot mint host shared memory".into(),
        ))
    }

    async fn export_snapshot(
        &self,
    ) -> Result<Box<dyn tokio::io::AsyncRead + Send + Sync + Unpin>, DriverError> {
        self.require_running()?;
        let data = self.ledger.to_image().into_bytes();
        Ok(Box::new(FixtureSnapshotReader { data, pos: 0 }))
    }

    async fn import_snapshot(
        &mut self,
        mut reader: Box<dyn tokio::io::AsyncRead + Send + Sync + Unpin>,
    ) -> Result<(), DriverError> {
        let mut text = String::new();
        reader
            .read_to_string(&mut text)
            .await
            .map_err(DriverError::Io)?;
        self.ledger = FixtureLedger::from_image(&text)?;
        self.phase = FixturePhase::Running;
        Ok(())
    }

    // see: cans/08-lifecycle.md#Rollback
    async fn rollback(&mut self, snapshot: &SnapshotDescriptor) -> Result<(), DriverError> {
        self.require_running()?;
        let Some(index) = self
            .ledger
            .snapshots
            .iter()
            .position(|descriptor| descriptor.id == snapshot.id)
        else {
            return Err(DriverError::InvalidState(format!(
                "rollback target `{}` is not in the fixture snapshot chain",
                snapshot.id
            )));
        };
        self.ledger.snapshots.truncate(index + 1);
        self.ledger.snapshot_seq = self.ledger.snapshots.len() as u64;
        Ok(())
    }

    // see: cans/08-lifecycle.md#Suspend-and-resume
    async fn suspend(
        &mut self,
        _reason: SuspendReason,
        output_path: &Path,
    ) -> Result<(), DriverError> {
        self.require_running()?;
        let image = self.ledger.to_image();
        std::fs::write(output_path, image).map_err(|e| {
            DriverError::Io(std::io::Error::other(format!(
                "fixture suspend image {}: {e}",
                output_path.display()
            )))
        })?;
        self.phase = FixturePhase::Suspended;
        Ok(())
    }

    // see: cans/08-lifecycle.md#Suspend-and-resume
    async fn resume(&mut self, image_path: &Path) -> Result<(), DriverError> {
        match self.phase {
            FixturePhase::Suspended => {}
            _ => {
                return Err(DriverError::InvalidState(
                    "fixture driver resumes only from the suspended phase".to_string(),
                ));
            }
        }
        let text = std::fs::read_to_string(image_path).map_err(|e| {
            DriverError::Io(std::io::Error::other(format!(
                "fixture suspend image {}: {e}",
                image_path.display()
            )))
        })?;
        self.ledger = FixtureLedger::from_image(&text)?;
        self.phase = FixturePhase::Running;
        Ok(())
    }

    // see: cans/08-lifecycle.md#Clock-skew
    async fn skew_clock(&mut self, offset: Duration) -> Result<(), DriverError> {
        self.require_running()?;
        self.ledger.clock_offset_ns += offset.as_nanos() as u64;
        Ok(())
    }

    // see: cans/08-lifecycle.md#Balloon
    async fn adjust_balloon(&mut self, target_mb: u32) -> Result<BalloonConvergence, DriverError> {
        self.require_running()?;
        self.ledger.balloon_mb = Some(target_mb);
        Ok(BalloonConvergence {
            target_mb,
            converged: true,
        })
    }

    // see: cans/10-attestation.md#Lifecycle-bridge
    async fn freeze_vcpus(&mut self) -> Result<(), DriverError> {
        self.require_running()?;
        if self.frozen_vcpus {
            return Err(DriverError::InvalidState(
                "fixture vCPUs are already frozen".to_string(),
            ));
        }
        self.frozen_vcpus = true;
        Ok(())
    }

    // see: cans/10-attestation.md#Lifecycle-bridge
    async fn unfreeze_vcpus(&mut self) -> Result<(), DriverError> {
        if !self.frozen_vcpus {
            return Err(DriverError::InvalidState(
                "fixture vCPUs are not frozen".to_string(),
            ));
        }
        self.frozen_vcpus = false;
        Ok(())
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    async fn drain_fuse(&mut self, timeout: Duration) -> Result<(), DriverError> {
        self.require_running()?;
        self.last_drain_timeout = Some(timeout);
        Ok(())
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    async fn commit_broker_upper(&mut self) -> Result<(), DriverError> {
        self.require_running()?;
        self.ledger.broker_upper_committed += 1;
        Ok(())
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    async fn discard_broker_upper(&mut self) -> Result<(), DriverError> {
        self.require_running()?;
        self.ledger.broker_upper_discarded += 1;
        Ok(())
    }

    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    async fn step_clock_window(&mut self, retired_instructions: u64) -> Result<(), DriverError> {
        self.require_running()?;
        self.ledger.retired_instructions += retired_instructions;
        Ok(())
    }

    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    fn register_pmu_sampler(&mut self, sampler: Arc<dyn PmuSampler>) -> Result<(), DriverError> {
        self.pmu_samplers.push(sampler);
        Ok(())
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn set_entropy_backend(&mut self, backend: EntropyBackend) -> Result<(), DriverError> {
        self.entropy_backend = Some(backend);
        Ok(())
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn entropy_position(&self) -> Result<EntropyStreamPosition, DriverError> {
        Ok(EntropyStreamPosition {
            offset_bytes: self.ledger.entropy_offset_bytes,
        })
    }

    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn restore_entropy_position(
        &mut self,
        position: EntropyStreamPosition,
    ) -> Result<(), DriverError> {
        self.ledger.entropy_offset_bytes = position.offset_bytes;
        Ok(())
    }

    // see: cans/08-lifecycle.md#Watchdog
    async fn set_watchdog(
        &mut self,
        timeout: Duration,
        action: WatchdogAction,
    ) -> Result<(), DriverError> {
        self.armed_watchdog = Some((timeout, action));
        Ok(())
    }

    // see: cans/01-architecture.md#Transport-engine-abstraction
    fn transport_connect(
        &self,
        channel_id: u32,
    ) -> Result<Box<dyn kelir_interface::driver_interface::Stream>, DriverError> {
        self.require_running()?;
        Ok(Box::new(crate::LoopbackStream::new(channel_id)))
    }

    // see: cans/08-lifecycle.md#Pre-teardown-flush
    async fn terminate(&mut self) -> Result<(), DriverError> {
        match self.phase {
            FixturePhase::Running | FixturePhase::Suspended => {
                self.phase = FixturePhase::Terminated;
                Ok(())
            }
            FixturePhase::Created => Err(DriverError::InvalidState(
                "fixture driver terminates only a booted VM".to_string(),
            )),
            FixturePhase::Terminated => Err(DriverError::InvalidState(
                "fixture driver has already terminated".to_string(),
            )),
        }
    }
}

// see: cans/02-plugin-model.md#Plugin-testkit
pub struct LifecycleCtxFixture {
    config: VmConfig,
    session_id: String,
    driver: FixtureVmmDriver,
    vetoes: VetoList,
}

impl Default for LifecycleCtxFixture {
    fn default() -> Self {
        Self::new()
    }
}

impl LifecycleCtxFixture {
    pub fn new() -> Self {
        Self {
            config: default_vm_config(None, None, None, None, None, None),
            session_id: String::new(),
            driver: FixtureVmmDriver::new(),
            vetoes: Vec::new(),
        }
    }

    pub fn ctx(&mut self) -> LifecycleCtx<'_> {
        LifecycleCtx::new(
            &self.config,
            &self.session_id,
            &mut self.driver,
            &mut self.vetoes,
        )
    }

    pub fn arm_watchdog(&mut self, action: WatchdogAction) {
        self.driver.armed_watchdog = Some((Duration::ZERO, action));
    }

    pub fn armed_watchdog(&self) -> Option<(Duration, WatchdogAction)> {
        self.driver.armed_watchdog.clone()
    }

    pub fn vetoes(&self) -> &VetoList {
        &self.vetoes
    }
}

pub fn lifecycle_ctx_fixture() -> LifecycleCtxFixture {
    LifecycleCtxFixture::new()
}

pub fn lifecycle_ctx_with_watchdog(action: WatchdogAction) -> LifecycleCtxFixture {
    let mut fixture = LifecycleCtxFixture::new();
    fixture.arm_watchdog(action);
    fixture
}
