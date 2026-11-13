// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/01-architecture.md#Foreign-runtime-embedding
// see: cans/02-plugin-model.md#Foreign-plugin-proxy
#![forbid(unsafe_code)]
use std::sync::Arc;
use std::time::Instant;

use napi::bindgen_prelude::Buffer;
use napi::bindgen_prelude::Object;
use napi::{Error, Result, Status};
use napi_derive::napi;
use tokio::sync::Mutex as AsyncMutex;

pub mod fusion_plugin;
pub mod storage_bridge;

use fusion_plugin::{FusedTrampolineDescriptor, TrampolineTable, attach_runs, convert_descriptor};
use storage_bridge::storage_config;

use kelir_engine::Session;
use kelir_engine::planes::storage::storage_reconciler::FsReconcileRequest;
use kelir_interface::driver_interface::{
    ExternalSuspendKind, GuestClock, SnapshotDescriptor, SuspendReason, VmConfig, default_vm_config,
};
use kelir_interface::error_envelope::ErrorEnvelope;
use kelir_interface::handles_lifecycle::VmHandle as CoreVmHandle;
use kelir_interface::hex_codec::decode_hex_fixed;
use kelir_interface::plugin_model::{PluginError, PluginRegistration};

// see: cans/03-sdk-architecture.md#Distribution-and-error-invariants
fn halt(error: PluginError) -> Error {
    envelope_error(&ErrorEnvelope::from(error))
}

// see: cans/03-sdk-architecture.md#Distribution-and-error-invariants
fn envelope_error(envelope: &ErrorEnvelope) -> Error {
    Error::new(
        Status::GenericFailure,
        serde_json::to_string(envelope).unwrap_or_else(|_| envelope.message.clone()),
    )
}

// see: cans/03-sdk-architecture.md#Session-surfaces
fn missing(reason: &str) -> Error {
    Error::new(Status::GenericFailure, reason.to_string())
}

// see: cans/08-lifecycle.md#Detached-lifecycle
#[napi]
pub fn relay_socket_path(session_id: String) -> Result<String> {
    let id = decode_hex_fixed::<16>(&session_id).map_err(|_| {
        missing(&format!(
            "attach: session id `{session_id}` is not 32 hex chars"
        ))
    })?;
    Session::relay_socket_path(&id)
        .map_err(halt)
        .map(|path| path.display().to_string())
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

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
#[napi(object)]
#[derive(Clone)]
pub struct NativePluginContextRecord {
    pub name: String,
    pub ctx: NativePluginContextSurface,
}

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
#[napi(object)]
#[derive(Clone)]
pub struct NativePluginContextSurface {
    pub enabled: bool,
}

// see: cans/09-snapshots.md#Archives-and-portability
#[napi(object)]
pub struct NativeSnapshotDescriptorRecord {
    pub id: String,
    pub dirty_bytes: f64,
    pub created_at_ns: f64,
    pub parent_digest: Option<String>,
    pub guest_clock: Option<NativeGuestClockRecord>,
}

// see: cans/09-snapshots.md#Archives-and-portability
fn snapshot_record(descriptor: &SnapshotDescriptor) -> NativeSnapshotDescriptorRecord {
    NativeSnapshotDescriptorRecord {
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
fn snapshot_descriptor(record: NativeSnapshotDescriptorRecord) -> Result<SnapshotDescriptor> {
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
    pub async fn checkpoint(&self, name: String) -> Result<NativeSnapshotDescriptorRecord> {
        let descriptor = self.vm.checkpoint(name).await.map_err(halt)?;
        Ok(snapshot_record(&descriptor))
    }

    #[napi]
    pub async fn rollback(&self, snapshot: NativeSnapshotDescriptorRecord) -> Result<()> {
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
        // TODO (L3) [bindings_ts]: hand the SharedMemoryHandle across the bridge via SCM_RIGHTS once Session::memory_handoff lands.; slop: handle never crosses, Buffer fabricated or missing
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
        // TODO (L3) [terminal_plane]: publish the TerminalNotice event once the reactive event bus lands.; slop: notice never published, terminal state unobserved
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
    pub async fn create(&self, label: String) -> Result<NativeSnapshotDescriptorRecord> {
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
        // TODO (L3) [pm_02_stream_mux_open_data_close_flags]: open the duplex channel once the mux SPSC rings carry stream frames.; slop: channel opened without open/data/close flags
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
    pub fn last(&self) -> Result<NativeSnapshotDescriptorRecord> {
        // TODO (L3) [lifecycle_checkpoint]: expose the active StepSlot once dispatch constructs HandlerCtx step stacks.; slop: step slot never constructed, hooks run without step
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
    session_dir: String,
    plugin_contexts: Vec<NativePluginContextRecord>,
}

impl NativeSessionHandle {
    // see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
    // see: cans/03-sdk-architecture.md#Service-lookup-across-languages
    fn from_session(session: Session, driver_name: String, boot_ms: f64) -> Result<Self> {
        let session_dir = session.session_dir().map_err(halt)?.display().to_string();
        let plugin_contexts = session
            .plugin_contexts()
            .into_iter()
            .map(|record| NativePluginContextRecord {
                name: record.name,
                ctx: NativePluginContextSurface {
                    enabled: record.ctx.enabled,
                },
            })
            .collect();
        let vm = session.vm_handle();
        Ok(Self {
            session: Arc::new(AsyncMutex::new(session)),
            driver_name,
            boot_ms,
            vm,
            session_dir,
            plugin_contexts,
        })
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

    // see: cans/03-sdk-architecture.md#Sandbox-high-level-facade
    #[napi(getter)]
    pub fn session_dir(&self) -> String {
        self.session_dir.clone()
    }

    // see: cans/03-sdk-architecture.md#Service-lookup-across-languages
    #[napi(getter)]
    pub fn plugin_contexts(&self) -> Vec<NativePluginContextRecord> {
        self.plugin_contexts.clone()
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
            session.fork().await.map_err(halt)?
        };
        Self::from_session(
            child,
            self.driver_name.clone(),
            started.elapsed().as_secs_f64() * 1000.0,
        )
    }

    #[napi]
    pub async fn spawn(&self, _binary: String, _argv: Vec<String>) -> Result<()> {
        // TODO (L3) [tp_09_exec_stream_sublane_stdio_fidelity]: spawn writes on the EXEC stream kind via the shim wire protocol.; slop: EXEC writes bypass sublane, stdio interleaved
        Err(missing(
            "spawn: EXEC stream dispatch lands with the shim wire protocol (L3)",
        ))
    }

    #[napi]
    pub async fn exec_in_guest(&self, _binary: String, _argv: Vec<String>) -> Result<()> {
        // TODO (L3) [tp_09_exec_stream_sublane_stdio_fidelity]: exec_in_guest rides the EXEC stream kind via the shim wire protocol.; slop: exec rides wrong stream kind, output lost
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
        // TODO (L3) [lc_03_step_evaluation_taint_rollback]: lineage-derived head rollback needs the dispatch PID lineage.; slop: rollback without lineage, wrong head restored
        Err(missing(
            "rollback: pass a snapshot; lineage-derived head rollback lands with PID tracking (L3)",
        ))
    }

    #[napi]
    pub async fn stop(&self) -> Result<()> {
        let mut session = self.session.lock().await;
        session.stop().await.map_err(halt)
    }

    #[napi]
    pub async fn terminate(&self) -> Result<()> {
        let mut session = self.session.lock().await;
        session.kill().map_err(halt)
    }

    #[napi]
    pub async fn console(&self) -> Result<()> {
        // TODO (L3) [terminal_plane]: attach the PTY console master once the terminal plane owns it.; slop: PTY never attached, console output lost
        Err(missing(
            "console: PTY master attach lands with the terminal plane console (L3)",
        ))
    }

    #[napi]
    pub async fn get_causal_delta_from_parent(&self) -> Result<()> {
        // TODO (L3) [ar_09_causal_delta_aggregation_merkle]: causal deltas are produced by plugin-ledger over the delta chain.; slop: deltas fabricated, no merkle aggregation
        Err(missing(
            "get_causal_delta_from_parent: causal deltas come from plugin-ledger (L3)",
        ))
    }

    #[napi]
    pub async fn handle_ingress(&self) -> Result<()> {
        // TODO (L3) [wire_plane]: wire-ingress parked connections surface once the wire plane owns the host listeners.; slop: parked connections never surface, listeners silent
        Err(missing(
            "handle_ingress: inbound connections park in the wire plane listeners (L3)",
        ))
    }
}

// see: cans/03-sdk-architecture.md#Session-control-surface
#[napi(object)]
pub struct NativeTerminalConfig {
    pub cols: Option<u16>,
    pub rows: Option<u16>,
    pub drop_osc52: Option<bool>,
    pub strip_title: Option<bool>,
    pub token_shield_max_bytes_total: Option<f64>,
    pub token_shield_max_lines_total: Option<f64>,
    pub token_shield_clamp_strategy: Option<String>,
    pub stream_engine: Option<String>,
}

// see: cans/03-sdk-architecture.md#Session-control-surface
#[napi(object)]
pub struct NativeStorageConfig {
    pub idmap_guest_uid: Option<f64>,
    pub idmap_host_uid: Option<f64>,
    pub shared_caches: Option<Vec<String>>,
    pub lock_broker: Option<bool>,
    pub synthetic_files: Option<Vec<NativeSyntheticFile>>,
    pub mask_read: Option<Vec<NativeMaskRule>>,
    pub encapsulation: Option<String>,
    pub scan_engine: Option<String>,
    pub carryover_bytes: Option<f64>,
}

// see: cans/05-storage-plane.md#Workspace-commit
#[napi(object)]
pub struct NativeSyntheticFile {
    pub path: String,
    pub content: String,
    pub mode: Option<f64>,
}

// see: cans/05-storage-plane.md#Workspace-commit
#[napi(object)]
pub struct NativeMaskRule {
    pub glob: String,
    pub pattern: String,
    pub replace: String,
}

// see: cans/07-wire-plane.md#DNS-policy-filter
#[napi(object)]
pub struct NativeVaultDescriptor {
    pub name: String,
    pub kind: String,
    pub audience: String,
    pub placeholder: String,
}

// see: cans/07-wire-plane.md#Port-forwarding
#[napi(object)]
pub struct NativeL4Proxy {
    pub listen_port: u16,
    pub destination: String,
    pub inject_mtls_cert_path: Option<String>,
    pub inject_mtls_key_path: Option<String>,
}

// see: cans/07-wire-plane.md#DNS-policy-filter
#[napi(object)]
pub struct NativeWireConfig {
    pub sinkhole_dns_except: Option<Vec<String>>,
    pub sinkhole_drop_all: Option<bool>,
    pub ssh_agent_bridge: Option<bool>,
    pub vaults: Option<Vec<NativeVaultDescriptor>>,
    pub l4_proxies: Option<Vec<NativeL4Proxy>>,
    pub tap_filter: Option<String>,
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
#[derive(Default)]
#[napi(object)]
pub struct NativeKelirOptions {
    pub vcpus: Option<u8>,
    pub memory_mb: Option<u32>,
    pub kernel: Option<String>,
    pub rootfs_erofs: Option<String>,
    pub tsc_locked: Option<bool>,
    pub enable_watchdog: Option<bool>,
    pub driver: Option<String>,
}

#[napi]
pub struct NativeKelirBuilder {
    config: VmConfig,
    driver_name: String,
    plugins: Vec<PluginRegistration>,
    fused: Option<FusedTrampolineDescriptor>,
    trampolines: TrampolineTable,
    terminal_cfg: Option<NativeTerminalConfig>,
    storage_cfg: Option<NativeStorageConfig>,
    wire_cfg: Option<NativeWireConfig>,
}

#[napi]
impl NativeKelirBuilder {
    #[napi(constructor)]
    pub fn new(options: Option<NativeKelirOptions>) -> Self {
        let options = options.unwrap_or_default();
        let mut config = default_vm_config(
            options.vcpus,
            options.memory_mb,
            options.kernel,
            options.rootfs_erofs,
            options.tsc_locked,
            options.enable_watchdog,
        );
        // see: cans/01-architecture.md#Driver-backends
        config.driver_policy.driver = options.driver.clone();
        Self {
            config,
            driver_name: options.driver.unwrap_or_else(|| "auto".to_string()),
            plugins: Vec::new(),
            fused: None,
            trampolines: TrampolineTable::default(),
            terminal_cfg: None,
            storage_cfg: None,
            wire_cfg: None,
        }
    }

    // see: cans/04-terminal-plane.md#Terminal-pipeline
    // TODO (L3) [bindings_ts]: prompt rules and the token-shield clamp callback stay TypeScript-side until threadsafe-function bridging lands.; slop: callbacks stay TS-side, silently dropped
    #[napi]
    pub fn set_terminal_config(&mut self, config: NativeTerminalConfig) {
        self.terminal_cfg = Some(config);
    }

    // see: cans/05-storage-plane.md#Workspace-commit
    // TODO (L3) [bindings_ts]: snoop on_write callbacks stay TypeScript-side until threadsafe-function bridging lands.; slop: snoop callbacks stay TS-side, writes unobserved
    #[napi]
    pub fn set_storage_config(&mut self, config: NativeStorageConfig) {
        self.storage_cfg = Some(config);
    }

    // see: cans/07-wire-plane.md#DNS-policy-filter
    #[napi]
    pub fn set_wire_config(&mut self, config: NativeWireConfig) {
        self.wire_cfg = Some(config);
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
    pub fn fuse_hooks(&mut self, rings: Vec<Object>, runs: Option<Vec<Object>>) -> Result<()> {
        if self.fused.is_some() {
            return Err(Error::from_reason(
                "fuse_hooks: the trampoline descriptor is frozen at seal".to_string(),
            ));
        }
        let mut descriptor = convert_descriptor(rings)?;
        if let Some(runs) = runs {
            attach_runs(&mut descriptor, runs)?;
        }
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
        // TODO (L3) [bt_03_typescript_fused_trampoline_single_crossing_chained_hooks]: rendezvous the JS return value into a Verdict and marshal the ring event payload across the threadsafe boundary; slop: JS hook runs fire-and-forget, its verdict never gates dispatch
        let plugins: Vec<PluginRegistration> = self.plugins.clone();
        let plane_wiring = self.plane_wiring()?;
        let synthetic_files = self
            .storage_cfg
            .as_ref()
            .map(storage_bridge::synthetic_files)
            .transpose()?
            .unwrap_or_default();
        let started = Instant::now();
        let session = Session::boot_with_plugins(
            self.config.clone(),
            &plugins,
            Some(descriptor),
            synthetic_files,
            plane_wiring,
        )
        .map_err(halt)?;
        // TODO (L3) [hypervisor_driver]: re-source driver_name from the booted core session once it exposes the resolved driver.; slop: driver name stale, wrong backend reported
        NativeSessionHandle::from_session(
            session,
            self.driver_name.clone(),
            started.elapsed().as_secs_f64() * 1000.0,
        )
    }
}

impl Default for NativeKelirBuilder {
    fn default() -> Self {
        Self::new(None)
    }
}

// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
impl NativeKelirBuilder {
    // TODO (L3) [bindings_ts]: token-shield clamps, prompt rules, snoop callbacks, and mtls injection ride threadsafe functions into the planes.; slop: plane wiring drops clamps, snoop, mtls
    fn plane_wiring(&self) -> Result<kelir_engine::session_lifecycle::PlaneWiring> {
        let terminal =
            self.terminal_cfg
                .as_ref()
                .map(|cfg| kelir_engine::planes::terminal::TerminalWiring {
                    cols: cfg.cols.unwrap_or(80),
                    rows: cfg.rows.unwrap_or(24),
                    drop_osc52: cfg.drop_osc52.unwrap_or(false),
                    strip_title: cfg.strip_title.unwrap_or(false),
                    token_shield: None,
                    prompts: Vec::new(),
                });
        let wire = self
            .wire_cfg
            .as_ref()
            .map(|cfg| kelir_engine::planes::wire::WireWiring {
                sinkhole_allowlist: cfg.sinkhole_dns_except.clone().unwrap_or_default(),
                sinkhole_drop_all: cfg.sinkhole_drop_all.unwrap_or(false),
                ssh_agent_bridge: cfg.ssh_agent_bridge.unwrap_or(false),
                vault_placeholders: cfg
                    .vaults
                    .as_ref()
                    .map(|vaults| {
                        vaults
                            .iter()
                            .map(|vault| vault.placeholder.clone())
                            .collect()
                    })
                    .unwrap_or_default(),
                l4_proxies: cfg
                    .l4_proxies
                    .as_ref()
                    .map(|proxies| {
                        proxies
                            .iter()
                            .map(|proxy| (proxy.listen_port, proxy.destination.clone()))
                            .collect()
                    })
                    .unwrap_or_default(),
                allow_udp_proxied: cfg.tap_filter.as_deref() == Some("AllowUdpProxied"),
            });
        let storage = match &self.storage_cfg {
            Some(cfg) => storage_config(cfg)?,
            None => kelir_engine::planes::storage::StorageConfig::default(),
        };
        Ok(kelir_engine::session_lifecycle::PlaneWiring {
            terminal,
            wire,
            storage,
        })
    }
}
