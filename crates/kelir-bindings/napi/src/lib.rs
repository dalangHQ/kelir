// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/01-architecture.md#Foreign-runtime-embedding
// see: cans/02-plugin-model.md#Foreign-plugin-proxy
use std::sync::Arc;
use std::time::Instant;

use napi::bindgen_prelude::Buffer;
use napi::bindgen_prelude::Object;
use napi::{Error, Result, Status};
use napi_derive::napi;
use tokio::sync::Mutex as AsyncMutex;

pub mod fusion_plugin;

use fusion_plugin::{FusedTrampolineDescriptor, TrampolineTable, convert_descriptor};

pub struct PluginRegistration {
    pub name: String,
    pub enabled: bool,
}

use kelir_core::Session;
use kelir_core::planes::storage::storage_reconciler::FsReconcileRequest;
use kelir_interface::driver_interface::{
    ExternalSuspendKind, GuestClock, SnapshotDescriptor, SuspendReason, VmConfig, default_vm_config,
};
use kelir_interface::handles_lifecycle::VmHandle as CoreVmHandle;
use kelir_interface::plugin_model::PluginError;

// see: cans/03-sdk-architecture.md#Session-surfaces
fn halt(error: PluginError) -> Error {
    Error::new(Status::GenericFailure, error.to_string())
}

// see: cans/03-sdk-architecture.md#Session-surfaces
fn missing(reason: &str) -> Error {
    Error::new(Status::GenericFailure, reason.to_string())
}

// see: cans/03-sdk-architecture.md#Session-surfaces
fn u64_of(value: f64, what: &str) -> Result<u64> {
    if !(value.is_finite() && value >= 0.0 && value <= u64::MAX as f64) {
        return Err(missing(what));
    }
    Ok(value as u64)
}

// see: cans/08-lifecycle.md#Suspend-and-resume
const SDK_SUSPEND_REASON: SuspendReason =
    SuspendReason::External(ExternalSuspendKind::HumanAuthorizationHold);

// see: cans/09-snapshots.md#Archives-and-portability
#[napi(object)]
pub struct NativeGuestClockRecord {
    pub absolute_wall_clock_ns: f64,
    pub monotonic_offset_ns: f64,
    pub timezone: String,
    pub boot_params: String,
}

// see: cans/09-snapshots.md#Archives-and-portability
#[napi(object)]
pub struct NativeSnapshotRecord {
    pub id: String,
    pub dirty_bytes: f64,
    pub created_at_ns: f64,
    pub parent_digest: Option<String>,
    pub guest_clock: Option<NativeGuestClockRecord>,
}

// see: cans/09-snapshots.md#Archives-and-portability
fn snapshot_record(descriptor: &SnapshotDescriptor) -> NativeSnapshotRecord {
    NativeSnapshotRecord {
        id: descriptor.id.clone(),
        dirty_bytes: descriptor.dirty_bytes as f64,
        created_at_ns: descriptor.created_at_ns as f64,
        parent_digest: descriptor.parent_digest.clone(),
        guest_clock: descriptor
            .guest_clock
            .as_ref()
            .map(|clock| NativeGuestClockRecord {
                absolute_wall_clock_ns: clock.absolute_wall_clock_ns as f64,
                monotonic_offset_ns: clock.monotonic_offset_ns as f64,
                timezone: clock.timezone.clone(),
                boot_params: clock.boot_params.clone(),
            }),
    }
}

// see: cans/09-snapshots.md#Archives-and-portability
fn guest_clock_of(record: NativeGuestClockRecord) -> Result<GuestClock> {
    Ok(GuestClock {
        absolute_wall_clock_ns: u64_of(
            record.absolute_wall_clock_ns,
            "guest clock absolute_wall_clock_ns is not a u64",
        )?,
        monotonic_offset_ns: u64_of(
            record.monotonic_offset_ns,
            "guest clock monotonic_offset_ns is not a u64",
        )?,
        timezone: record.timezone,
        boot_params: record.boot_params,
    })
}

// see: cans/09-snapshots.md#Archives-and-portability
fn snapshot_descriptor(record: NativeSnapshotRecord) -> Result<SnapshotDescriptor> {
    Ok(SnapshotDescriptor {
        id: record.id,
        dirty_bytes: u64_of(record.dirty_bytes, "snapshot dirty_bytes is not a u64")?,
        created_at_ns: u64_of(record.created_at_ns, "snapshot created_at_ns is not a u64")?,
        parent_digest: record.parent_digest,
        guest_clock: record.guest_clock.map(guest_clock_of).transpose()?,
    })
}

// see: cans/03-sdk-architecture.md#Session-control-surface
#[napi(object)]
pub struct NativeClockSkewOptions {
    pub days: Option<f64>,
    pub hours: Option<f64>,
    pub ms: Option<f64>,
}

// see: cans/03-sdk-architecture.md#Session-control-surface
fn skew_duration(options: NativeClockSkewOptions) -> Result<std::time::Duration> {
    let total_ms = options.days.unwrap_or(0.0) * 86_400_000.0
        + options.hours.unwrap_or(0.0) * 3_600_000.0
        + options.ms.unwrap_or(0.0);
    Ok(std::time::Duration::from_millis(u64_of(
        total_ms,
        "clock skew is not a finite non-negative duration",
    )?))
}

// see: cans/05-storage-plane.md#Workspace-commit
#[napi(object)]
pub struct NativeReconcileOptions {
    pub cwd: String,
    pub atomic_staging: Option<bool>,
    pub fsync: Option<bool>,
}

// see: cans/05-storage-plane.md#Workspace-commit
#[napi(object)]
pub struct NativeCommitStatsRecord {
    pub files_changed: f64,
    pub tombstones_resolved: f64,
}

// see: cans/04-terminal-plane.md#Terminal-pipeline
#[napi(object)]
pub struct NativeTerminalReadRecord {
    pub bytes: Buffer,
    pub next_offset: f64,
}

// see: cans/08-lifecycle.md#Clock-skew
#[napi]
pub struct NativeClockHandle {
    clock: kelir_interface::handles_lifecycle::ClockHandle,
}

#[napi]
impl NativeClockHandle {
    #[napi]
    pub async fn skew(&self, options: NativeClockSkewOptions) -> Result<()> {
        self.clock.skew(skew_duration(options)?).await.map_err(halt)
    }
}

// see: cans/03-sdk-architecture.md#Session-control-surface
#[napi]
pub struct NativeVmHandle {
    vm: CoreVmHandle,
}

#[napi]
impl NativeVmHandle {
    #[napi]
    pub async fn checkpoint(&self, name: String) -> Result<NativeSnapshotRecord> {
        let descriptor = self.vm.checkpoint(name).await.map_err(halt)?;
        Ok(snapshot_record(&descriptor))
    }

    #[napi]
    pub async fn rollback(&self, snapshot: NativeSnapshotRecord) -> Result<()> {
        self.vm
            .rollback(&snapshot_descriptor(snapshot)?)
            .await
            .map_err(halt)
    }

    #[napi]
    pub async fn suspend(&self, path: String) -> Result<()> {
        self.vm
            .suspend(SDK_SUSPEND_REASON, &path)
            .await
            .map_err(halt)
    }

    #[napi]
    pub async fn resume(&self, path: String) -> Result<()> {
        self.vm.resume(&path).await.map_err(halt)
    }

    #[napi(getter)]
    pub fn clock(&self) -> NativeClockHandle {
        NativeClockHandle {
            clock: self.vm.clock.clone(),
        }
    }

    #[napi]
    pub async fn adjust_balloon(&self, megabytes: u32) -> Result<()> {
        self.vm.adjust_balloon(megabytes).await.map_err(halt)?;
        Ok(())
    }

    #[napi]
    pub async fn set_vcpus(&self, count: u32) -> Result<()> {
        let count = u8::try_from(count).map_err(|_| missing("vCPU count is not a u8"))?;
        self.vm.set_vcpus(count).await.map_err(halt)
    }

    #[napi]
    pub async fn map_shared_region(&self, _tag: String, _size_bytes: f64) -> Result<Buffer> {
        // TODO (L3): hand the SharedMemoryHandle across the bridge via SCM_RIGHTS once Session::memory_handoff lands.
        Err(missing(
            "map_shared_region: the shared-memory handle crosses via SCM_RIGHTS handoff (L3)",
        ))
    }

    #[napi]
    pub async fn export_snapshot(&self) -> Result<Buffer> {
        let bytes = self.vm.export_snapshot().await.map_err(halt)?;
        Ok(Buffer::from(bytes))
    }

    #[napi]
    pub async fn import_snapshot(&self, data: Buffer) -> Result<()> {
        self.vm.import_snapshot(&data).await.map_err(halt)
    }
}

// see: cans/04-terminal-plane.md#Terminal-pipeline
#[napi]
pub struct NativeTerminalHandle {
    session: Arc<AsyncMutex<Session>>,
}

#[napi]
impl NativeTerminalHandle {
    #[napi]
    pub fn notice(&self, _message: String) -> Result<()> {
        // TODO (L3): publish the TerminalNotice event once the reactive event bus lands.
        Err(missing(
            "terminal notice: TerminalNotice emission lands with the reactive event bus (L3)",
        ))
    }

    #[napi]
    pub async fn resize(&self, cols: u32, rows: u32) -> Result<()> {
        let cols = u16::try_from(cols).map_err(|_| missing("terminal cols is not a u16"))?;
        let rows = u16::try_from(rows).map_err(|_| missing("terminal rows is not a u16"))?;
        let session = self.session.lock().await;
        session.terminal.resize(cols, rows).map_err(halt)
    }

    #[napi]
    pub async fn export_screen_matrix(&self) -> Result<Buffer> {
        let session = self.session.lock().await;
        let bytes = session.terminal.export_screen_matrix().map_err(halt)?;
        Ok(Buffer::from(bytes))
    }

    #[napi]
    pub async fn read_from(&self, port: u32, offset: f64) -> Result<NativeTerminalReadRecord> {
        let cursor = u64_of(offset, "terminal offset is not a u64")?;
        let session = self.session.lock().await;
        let (bytes, next_offset) = session.terminal.read_from(port, cursor).map_err(halt)?;
        Ok(NativeTerminalReadRecord {
            bytes: Buffer::from(bytes),
            next_offset: next_offset as f64,
        })
    }
}

// see: cans/05-storage-plane.md#Workspace-commit
#[napi]
pub struct NativeStorageHandle {
    session: Arc<AsyncMutex<Session>>,
}

#[napi]
impl NativeStorageHandle {
    #[napi]
    pub async fn reconcile(
        &self,
        options: NativeReconcileOptions,
    ) -> Result<NativeCommitStatsRecord> {
        let session = self.session.lock().await;
        let stats = session
            .storage
            .reconcile(FsReconcileRequest {
                cwd: options.cwd,
                atomic_staging: options.atomic_staging.unwrap_or(false),
                fsync: options.fsync.unwrap_or(false),
            })
            .map_err(halt)?;
        Ok(NativeCommitStatsRecord {
            files_changed: stats.files_changed as f64,
            tombstones_resolved: stats.tombstones_resolved as f64,
        })
    }

    #[napi]
    pub async fn discard(&self) -> Result<()> {
        let session = self.session.lock().await;
        session.storage.discard().map_err(halt)
    }
}

// see: cans/07-wire-plane.md#Port-forwarding
#[napi]
pub struct NativeWireHandle {
    session: Arc<AsyncMutex<Session>>,
}

#[napi]
impl NativeWireHandle {
    #[napi]
    pub async fn forward_port(&self, guest_port: u16, host_port: u16) -> Result<f64> {
        let session = self.session.lock().await;
        let allocated = session
            .wire
            .forward_port(guest_port, Some(host_port))
            .map_err(halt)?;
        Ok(f64::from(allocated))
    }
}

// see: cans/09-snapshots.md#Snapshot-taxonomy
#[napi]
pub struct NativeCheckpointHandle {
    vm: CoreVmHandle,
}

#[napi]
impl NativeCheckpointHandle {
    #[napi]
    pub async fn create(&self, label: String) -> Result<NativeSnapshotRecord> {
        let descriptor = self.vm.checkpoint(label).await.map_err(halt)?;
        Ok(snapshot_record(&descriptor))
    }
}

// see: cans/01-architecture.md#Host-state-root
#[napi]
pub struct NativeHostHandle {
    session: Arc<AsyncMutex<Session>>,
}

#[napi]
impl NativeHostHandle {
    #[napi]
    pub async fn lsm_status(&self) -> Result<bool> {
        let session = self.session.lock().await;
        Ok(session.host.lsm_status())
    }

    #[napi]
    pub async fn pressure_level(&self) -> Result<f64> {
        let session = self.session.lock().await;
        Ok(f64::from(session.host.pressure_level()))
    }
}

// see: cans/11-protocol.md#Control-channel
#[napi]
pub struct NativeVsockHandle {}

#[napi]
impl NativeVsockHandle {
    #[napi]
    pub async fn channel(&self, _kind: u32) -> Result<()> {
        // TODO (L3): open the duplex channel once the mux SPSC rings carry stream frames.
        Err(missing(
            "vsock channel: duplex streams ride the mux SPSC rings (L3)",
        ))
    }
}

// see: cans/03-sdk-architecture.md#Session-data-operations
#[napi]
pub struct NativeStepHandle {}

#[napi]
impl NativeStepHandle {
    #[napi]
    pub fn last(&self) -> Result<NativeSnapshotRecord> {
        // TODO (L3): expose the active StepSlot once dispatch constructs HandlerCtx step stacks.
        Err(missing(
            "step last: step slots live in the dispatch HandlerCtx (L3)",
        ))
    }
}

// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
#[napi]
pub struct NativeSessionHandle {
    session: Arc<AsyncMutex<Session>>,
    driver_name: String,
    boot_ms: f64,
    vm: CoreVmHandle,
}

impl NativeSessionHandle {
    // see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
    fn from_session(session: Session, driver_name: String, boot_ms: f64) -> Self {
        let vm = session.vm_handle();
        Self {
            session: Arc::new(AsyncMutex::new(session)),
            driver_name,
            boot_ms,
            vm,
        }
    }
}

#[napi]
impl NativeSessionHandle {
    #[napi(getter)]
    pub fn driver_name(&self) -> String {
        self.driver_name.clone()
    }

    #[napi(getter)]
    pub fn boot_ms(&self) -> f64 {
        self.boot_ms
    }

    #[napi(getter)]
    pub fn vm(&self) -> NativeVmHandle {
        NativeVmHandle {
            vm: self.vm.clone(),
        }
    }

    #[napi(getter)]
    pub fn terminal(&self) -> NativeTerminalHandle {
        NativeTerminalHandle {
            session: Arc::clone(&self.session),
        }
    }

    #[napi(getter)]
    pub fn storage(&self) -> NativeStorageHandle {
        NativeStorageHandle {
            session: Arc::clone(&self.session),
        }
    }

    #[napi(getter)]
    pub fn wire(&self) -> NativeWireHandle {
        NativeWireHandle {
            session: Arc::clone(&self.session),
        }
    }

    #[napi(getter)]
    pub fn checkpoint(&self) -> NativeCheckpointHandle {
        NativeCheckpointHandle {
            vm: self.vm.clone(),
        }
    }

    #[napi(getter)]
    pub fn host(&self) -> NativeHostHandle {
        NativeHostHandle {
            session: Arc::clone(&self.session),
        }
    }

    #[napi(getter)]
    pub fn vsock(&self) -> NativeVsockHandle {
        NativeVsockHandle {}
    }

    #[napi(getter)]
    pub fn step(&self) -> NativeStepHandle {
        NativeStepHandle {}
    }

    #[napi]
    pub async fn fork(&self) -> Result<NativeSessionHandle> {
        let started = Instant::now();
        let child = {
            let session = self.session.lock().await;
            session.fork().map_err(halt)?
        };
        Ok(Self::from_session(
            child,
            self.driver_name.clone(),
            started.elapsed().as_secs_f64() * 1000.0,
        ))
    }

    #[napi]
    pub async fn spawn(&self, _binary: String, _argv: Vec<String>) -> Result<()> {
        // TODO (L3): spawn writes on the EXEC stream kind via the shim wire protocol.
        Err(missing(
            "spawn: EXEC stream dispatch lands with the shim wire protocol (L3)",
        ))
    }

    #[napi]
    pub async fn exec_in_guest(&self, _binary: String, _argv: Vec<String>) -> Result<()> {
        // TODO (L3): exec_in_guest rides the EXEC stream kind via the shim wire protocol.
        Err(missing(
            "exec_in_guest: EXEC stream dispatch lands with the shim wire protocol (L3)",
        ))
    }

    #[napi]
    pub async fn suspend(&self, path: String) -> Result<()> {
        self.vm
            .suspend(SDK_SUSPEND_REASON, &path)
            .await
            .map_err(halt)
    }

    #[napi]
    pub async fn resume(&self, path: String) -> Result<()> {
        self.vm.resume(&path).await.map_err(halt)
    }

    #[napi]
    pub async fn rollback(&self) -> Result<()> {
        // TODO (L3): lineage-derived head rollback needs the dispatch PID lineage.
        Err(missing(
            "rollback: pass a snapshot; lineage-derived head rollback lands with PID tracking (L3)",
        ))
    }

    #[napi]
    pub async fn stop(&self) -> Result<()> {
        let mut session = self.session.lock().await;
        session.stop().map_err(halt)
    }

    #[napi]
    pub async fn terminate(&self) -> Result<()> {
        let mut session = self.session.lock().await;
        session.kill().map_err(halt)
    }

    #[napi]
    pub async fn console(&self) -> Result<()> {
        // TODO (L3): attach the PTY console master once the terminal plane owns it.
        Err(missing(
            "console: PTY master attach lands with the terminal plane console (L3)",
        ))
    }

    #[napi]
    pub async fn get_causal_delta_from_parent(&self) -> Result<()> {
        // TODO (L3): causal deltas are produced by plugin-ledger over the delta chain.
        Err(missing(
            "get_causal_delta_from_parent: causal deltas come from plugin-ledger (L3)",
        ))
    }

    #[napi]
    pub async fn handle_ingress(&self) -> Result<()> {
        // TODO (L3): wire-ingress parked connections surface once the wire plane owns the host listeners.
        Err(missing(
            "handle_ingress: inbound connections park in the wire plane listeners (L3)",
        ))
    }
}

#[napi]
pub struct NativeKelirBuilder {
    config: VmConfig,
    driver_name: String,
    plugins: Vec<PluginRegistration>,
    fused: Option<FusedTrampolineDescriptor>,
    trampolines: TrampolineTable,
}

#[napi]
impl NativeKelirBuilder {
    #[napi(constructor)]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        vcpus: Option<u8>,
        memory_mb: Option<u32>,
        kernel: Option<String>,
        rootfs_erofs: Option<String>,
        tsc_locked: Option<bool>,
        enable_watchdog: Option<bool>,
        driver: Option<String>,
    ) -> Self {
        Self {
            config: default_vm_config(
                vcpus,
                memory_mb,
                kernel,
                rootfs_erofs,
                tsc_locked,
                enable_watchdog,
            ),
            driver_name: driver.unwrap_or_else(|| "auto".to_string()),
            plugins: Vec::new(),
            fused: None,
            trampolines: TrampolineTable::default(),
        }
    }

    #[napi]
    pub fn add_plugin(&mut self, name: String, enabled: Option<bool>) -> Result<()> {
        if name.trim().is_empty() {
            return Err(Error::new(
                Status::InvalidArg,
                "add_plugin: plugin name must not be empty".to_string(),
            ));
        }
        if self.plugins.iter().any(|plugin| plugin.name == name) {
            return Ok(());
        }
        self.plugins.push(PluginRegistration {
            name,
            enabled: enabled.unwrap_or(true),
        });
        Ok(())
    }

    #[napi]
    pub fn fuse_hooks(&mut self, rings: Vec<Object>) -> Result<()> {
        if self.fused.is_some() {
            return Err(Error::from_reason(
                "fuse_hooks: the trampoline descriptor is frozen at seal".to_string(),
            ));
        }
        let descriptor = convert_descriptor(rings)?;
        self.trampolines =
            TrampolineTable::from_descriptor(&descriptor).map_err(Error::from_reason)?;
        self.fused = Some(descriptor);
        Ok(())
    }

    #[napi]
    pub fn trampoline_entry_count(&self) -> u32 {
        self.trampolines.entry_count()
    }

    #[napi]
    pub fn boot(&self) -> Result<NativeSessionHandle> {
        let Some(descriptor) = &self.fused else {
            return Err(Error::from_reason(
                "boot: fuse_hooks must run before boot".to_string(),
            ));
        };
        if descriptor.rings.len() as u32 != self.trampolines.entry_count() {
            return Err(Error::from_reason(
                "boot: the trampoline table is not one entry per ring event".to_string(),
            ));
        }
        for chain in &descriptor.rings {
            for hook in &chain.hooks {
                if let Some(plugin_name) = &hook.plugin_name {
                    match self
                        .plugins
                        .iter()
                        .find(|plugin| &plugin.name == plugin_name)
                    {
                        Some(registration) if registration.enabled => {}
                        Some(_) => {
                            return Err(Error::from_reason(format!(
                                "boot: fused ring {} carries hooks of disabled plugin `{plugin_name}`",
                                chain.ring.as_tag()
                            )));
                        }
                        None => {
                            return Err(Error::from_reason(format!(
                                "boot: fused ring {} carries hooks of unregistered plugin `{plugin_name}`",
                                chain.ring.as_tag()
                            )));
                        }
                    }
                }
            }
        }
        if let Some(reason) = self.config.missing_boot_path() {
            return Err(Error::new(Status::InvalidArg, reason.to_string()));
        }
        // TODO (L3): bind each fused ring chain to its single trampoline entry as one threadsafe closure and dispatch through the kelir-core pipeline.
        let plugins: Vec<kelir_core::PluginRegistration> = self
            .plugins
            .iter()
            .map(|plugin| kelir_core::PluginRegistration::new(plugin.name.clone(), plugin.enabled))
            .collect();
        let started = Instant::now();
        let session = Session::boot_with_plugins(self.config.clone(), &plugins)
            .map_err(|error| Error::from_reason(error.to_string()))?;
        // TODO (L3): re-source driver_name from the booted core session once it exposes the resolved driver.
        Ok(NativeSessionHandle::from_session(
            session,
            self.driver_name.clone(),
            started.elapsed().as_secs_f64() * 1000.0,
        ))
    }
}

impl Default for NativeKelirBuilder {
    fn default() -> Self {
        Self::new(None, None, None, None, None, None, None)
    }
}
