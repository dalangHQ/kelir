// see: cans/08-lifecycle.md#Session-and-context-handles
// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
// see: cans/14-observability.md#Metrics-pipeline
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use kelir_interface::handles_lifecycle::{DriverVmControl, VmHandle};
use kelir_interface::hex_codec::encode_hex;
use kelir_interface::lifecycle_state::LifecycleState;
use kelir_interface::plugin_model::PipelineBinder;
use kelir_interface::vsock_mesh::VAULT_METADATA_VSOCK_PORT;

// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
#[derive(Debug, Clone, Default)]
pub struct PlaneWiring {
    pub terminal: Option<crate::planes::terminal::TerminalWiring>,
    pub wire: Option<crate::planes::wire::WireWiring>,
    pub lock_broker: Option<crate::planes::storage::LockBrokerWiring>,
}

pub struct Session {
    pub terminal: Arc<crate::planes::terminal::TerminalPlane>,
    pub storage: crate::planes::storage::StoragePlane,
    pub wire: crate::planes::wire::WirePlane,
    pub checkpoint: crate::planes::checkpoint::CheckpointPlane,
    pub host: crate::planes::host::HostPlane,
    metrics: std::sync::Arc<kelir_otlp::registry_otlp::MetricRegistry>,
    uart: crate::uart_terminal::UartRingBuffer,
    driver: Option<
        Arc<
            tokio::sync::Mutex<Box<dyn kelir_interface::driver_interface::VmmDriver + Send + Sync>>,
        >,
    >,
    owned_bus: crate::bus::OwnedBusDispatcher,
    engine: crate::dispatch_plugin::PipelineEngine,
    session_id: [u8; 16],
    incarnation_id: [u8; 16],
    plugins: Vec<crate::plugin_registry::PluginRegistration>,
    detached: bool,
    state: LifecycleState,
}

// see: cans/08-lifecycle.md#Session-incarnation
fn mint_uuid_v7() -> [u8; 16] {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = u64::from(std::process::id());
    let mut id = [0u8; 16];
    let ts_ms = elapsed.as_millis() as u64 & 0x0000_FFFF_FFFF_FFFF;
    id[0..6].copy_from_slice(&ts_ms.to_be_bytes()[2..8]);
    let rand_a = counter & 0x0FFF;
    id[6] = 0x70 | (rand_a >> 8) as u8;
    id[7] = (rand_a & 0xFF) as u8;
    let rand_b =
        ((u64::from(elapsed.subsec_nanos()) << 20) ^ (pid << 8) ^ counter) & 0x3FFF_FFFF_FFFF_FFFF;
    id[8] = 0x80 | (rand_b >> 56) as u8;
    id[9..16].copy_from_slice(&(rand_b & 0x00FF_FFFF_FFFF_FFFF).to_be_bytes()[1..8]);
    id
}

// see: cans/05-storage-plane.md#Storage-quotas
const DEFAULT_WORKSPACE_QUOTA_BYTES: u64 = 512 * 1024 * 1024;

// see: cans/05-storage-plane.md#Storage-quotas
const DEFAULT_WORKSPACE_QUOTA_INODES: u64 = 65_536;

impl Session {
    // see: cans/08-lifecycle.md#Detached-lifecycle
    pub fn relay_socket_path(
        session_id: &[u8; 16],
    ) -> Result<std::path::PathBuf, kelir_interface::plugin_model::PluginError> {
        let home = std::env::var("HOME").map_err(|_| {
            kelir_interface::plugin_model::PluginError::Halt(
                "HOME is not set; cannot locate ~/.kelir relay socket".into(),
            )
        })?;
        Ok(std::path::PathBuf::from(home)
            .join(".kelir")
            .join("sessions")
            .join(encode_hex(session_id))
            .join("relay.sock"))
    }

    // see: cans/08-lifecycle.md#Detached-lifecycle
    pub fn attach(
        session_id: &[u8; 16],
    ) -> Result<Self, kelir_interface::plugin_model::PluginError> {
        let relay = Self::relay_socket_path(session_id)?;
        // TODO (L3): connect to relay.sock, verify the catalog lease is live, and rebuild the session from the runner-owned state.
        let _ = relay;
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): Session::attach".into(),
        ))
    }

    pub fn is_detached(&self) -> bool {
        self.detached
    }

    // see: cans/06-storage-catalog.md#Entity-model
    pub fn session_id(&self) -> [u8; 16] {
        self.session_id
    }

    pub fn set_detached(&mut self, detached: bool) {
        self.detached = detached;
    }

    // see: cans/08-lifecycle.md#Detached-lifecycle
    pub fn memory_handoff(
        &self,
        handoff: kelir_transport::MemoryHandoff,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3): pass the shared-memory descriptor via SCM_RIGHTS and close the parent end after the handoff ack.
        let _ = handoff;
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): Session::memory_handoff".into(),
        ))
    }
}

impl Session {
    pub fn boot(
        config: kelir_interface::driver_interface::VmConfig,
    ) -> Result<Self, kelir_interface::plugin_model::PluginError> {
        Self::boot_with_plugins(config, &[], None, PlaneWiring::default())
    }

    // see: cans/02-plugin-model.md#Foreign-plugin-proxy
    pub fn boot_with_plugins(
        config: kelir_interface::driver_interface::VmConfig,
        plugins: &[crate::plugin_registry::PluginRegistration],
        trampoline: Option<&kelir_interface::fusion_plugin::FusedTrampolineDescriptor>,
        plane_wiring: PlaneWiring,
    ) -> Result<Self, kelir_interface::plugin_model::PluginError> {
        let mut engine = crate::dispatch_plugin::PipelineEngine::new();
        if let Some(descriptor) = trampoline {
            engine = engine.with_trampoline(descriptor.clone());
        }
        Self::boot_with_engine(
            config,
            plugins,
            engine,
            std::sync::Arc::new(kelir_otlp::registry_otlp::MetricRegistry::new()),
            Vec::new(),
            plane_wiring,
        )
    }

    // see: cans/08-lifecycle.md#Hypervisor-primitives
    #[allow(clippy::too_many_arguments)]
    pub fn boot_with_engine(
        config: kelir_interface::driver_interface::VmConfig,
        plugins: &[crate::plugin_registry::PluginRegistration],
        engine: crate::dispatch_plugin::PipelineEngine,
        metrics: std::sync::Arc<kelir_otlp::registry_otlp::MetricRegistry>,
        synthetic_files: Vec<crate::planes::storage::storage_synthetic::SyntheticFile>,
        plane_wiring: PlaneWiring,
    ) -> Result<Self, kelir_interface::plugin_model::PluginError> {
        if let Some(reason) = config.missing_boot_path() {
            return Err(kelir_interface::plugin_model::PluginError::Halt(
                reason.into(),
            ));
        }
        let resolved = crate::plugin_registry::resolve_plugin_registrations(plugins)?;
        let incarnation_id = mint_uuid_v7();
        // see: cans/06-storage-catalog.md#Entity-model
        let session_id = mint_uuid_v7();
        let boot_id = encode_hex(&mint_uuid_v7());
        let start_time_ns = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default(),
        )
        .unwrap_or(i64::MAX);
        // TODO (L3): swap the minted boot_id and now-based start_time_ns for the OS host_boot_id and proc start_time per cans/08-lifecycle.md#Cross-platform-incarnation-proof.
        // TODO (L3): detached boots upgrade the row to lease_kind='lease' with runner child pid and expires_at per cans/08-lifecycle.md#Detached-lifecycle.
        let row = kelir_catalog::schema_catalog::SessionRow {
            session_id,
            incarnation_id: kelir_catalog::schema_catalog::IncarnationId::from_bytes(
                incarnation_id,
            ),
            lease_kind: kelir_catalog::schema_catalog::LeaseKind::Ephemeral,
            expires_at_unix: None,
            runner_pid: i64::from(std::process::id()),
            boot_id,
            start_time_ns,
        };
        kelir_catalog::CatalogDb::open(kelir_catalog::catalog_path().map_err(|error| {
            kelir_interface::plugin_model::PluginError::Halt(format!("{error}"))
        })?)
        .map_err(kelir_catalog::CatalogError::from)
        .and_then(|catalog| catalog.register_session(&row))
        .map_err(|error| {
            kelir_interface::plugin_model::PluginError::Halt(format!(
                "catalog session registration: {error}"
            ))
        })?;
        let session_hex = encode_hex(&incarnation_id);
        let workspace_root = std::env::temp_dir()
            .join("kelir")
            .join(&session_hex)
            .join("workspace");
        let storage = crate::planes::storage::StoragePlane::new(
            workspace_root.clone(),
            workspace_root.join("upper.img"),
            DEFAULT_WORKSPACE_QUOTA_BYTES,
            DEFAULT_WORKSPACE_QUOTA_INODES,
        )
        .with_synthetic_files(synthetic_files)
        .with_lock_broker(plane_wiring.lock_broker.unwrap_or_default());
        let wire = match plane_wiring.wire {
            Some(wiring) => crate::planes::wire::WirePlane::new().with_wiring(wiring),
            None => crate::planes::wire::WirePlane::new(),
        };
        if let Some(handler) = engine.vsock_service(VAULT_METADATA_VSOCK_PORT) {
            wire.bind_imds_sink(handler)?;
        }
        let terminal = Arc::new(
            match &plane_wiring.terminal {
                Some(wiring) => {
                    crate::planes::terminal::TerminalPlane::new(session_id).with_wiring(wiring)
                }
                None => crate::planes::terminal::TerminalPlane::new(session_id),
            }
            .with_metrics(metrics.clone()),
        );
        let engine = engine
            .with_metrics(metrics.clone())
            .with_terminal(Arc::clone(&terminal));
        metrics.set_active_vcpus(u32::from(config.vcpus));
        // TODO (L3): DriverRouter probes KVM/HVF/AVF and completes the driver boot handshake; vm_handle stays unbound until a backend loads.
        let driver = crate::vmm::DriverRouter::load_native_driver(
            config.driver_policy.driver.as_deref(),
            matches!(
                config.driver_policy.wx_enforcement,
                kelir_interface::driver_interface::WxPolicy::Strict
            ),
            config.driver_policy.allow_emulated_fallback,
        )
        .ok()
        .map(|loaded| Arc::new(tokio::sync::Mutex::new(loaded)));
        let mut session = Self {
            terminal,
            storage: storage.with_metrics(metrics.clone()),
            wire: wire.with_metrics(metrics.clone()),
            checkpoint: crate::planes::checkpoint::CheckpointPlane::new()
                .with_metrics(metrics.clone()),
            host: crate::planes::host::HostPlane::new().with_metrics(metrics.clone()),
            metrics,
            uart: crate::uart_terminal::UartRingBuffer::new(),
            driver,
            owned_bus: crate::bus::OwnedBusDispatcher::new(
                crate::bus::CONTROL_RING_CAPACITY,
                crate::bus::MUX_RING_CAPACITY,
            ),
            engine,
            session_id,
            incarnation_id,
            plugins: resolved,
            detached: false,
            state: LifecycleState::Initializing,
        };
        session.advance_lifecycle(LifecycleState::Running)?;
        Ok(session)
    }

    pub fn plugins(&self) -> &[crate::plugin_registry::PluginRegistration] {
        &self.plugins
    }

    // TODO (L2): from_parts is superseded once boot() binds real drivers and rings.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        terminal: crate::planes::terminal::TerminalPlane,
        storage: crate::planes::storage::StoragePlane,
        wire: crate::planes::wire::WirePlane,
        checkpoint: crate::planes::checkpoint::CheckpointPlane,
        host: crate::planes::host::HostPlane,
        driver: Arc<
            tokio::sync::Mutex<Box<dyn kelir_interface::driver_interface::VmmDriver + Send + Sync>>,
        >,
        owned_bus: crate::bus::OwnedBusDispatcher,
        engine: crate::dispatch_plugin::PipelineEngine,
        session_id: [u8; 16],
        incarnation_id: [u8; 16],
        plugins: Vec<crate::plugin_registry::PluginRegistration>,
        metrics: std::sync::Arc<kelir_otlp::registry_otlp::MetricRegistry>,
    ) -> Self {
        let terminal = Arc::new(terminal.with_metrics(metrics.clone()));
        let engine = engine
            .with_metrics(metrics.clone())
            .with_terminal(Arc::clone(&terminal));
        let mut session = Self {
            terminal,
            storage: storage.with_metrics(metrics.clone()),
            wire: wire.with_metrics(metrics.clone()),
            checkpoint: checkpoint.with_metrics(metrics.clone()),
            host: host.with_metrics(metrics.clone()),
            metrics: metrics.clone(),
            uart: crate::uart_terminal::UartRingBuffer::new(),
            driver: Some(driver),
            owned_bus,
            engine,
            session_id,
            incarnation_id,
            plugins,
            detached: false,
            state: LifecycleState::Initializing,
        };
        let _ = session.advance_lifecycle(LifecycleState::Running);
        session
    }

    pub fn metrics(&self) -> &std::sync::Arc<kelir_otlp::registry_otlp::MetricRegistry> {
        &self.metrics
    }

    pub fn pipeline(&self) -> &crate::dispatch_plugin::PipelineEngine {
        &self.engine
    }

    pub fn pipeline_mut(&mut self) -> &mut crate::dispatch_plugin::PipelineEngine {
        &mut self.engine
    }

    // see: cans/02-plugin-model.md#Core-context-keys
    pub fn vm_handle(&self) -> VmHandle {
        // TODO (L3): unbound until the native driver loads; driver ops fail with the typed unbound error.
        match &self.driver {
            Some(driver) => VmHandle::new(DriverVmControl::shared(driver)),
            None => VmHandle::default(),
        }
    }

    // see: cans/07-wire-plane.md#Pre-policy-drop-zones
    pub fn bind_wire_imds_vault(
        &mut self,
    ) -> Result<bool, kelir_interface::plugin_model::PluginError> {
        match self.engine.vsock_service(VAULT_METADATA_VSOCK_PORT) {
            Some(handler) => {
                self.wire.bind_imds_sink(handler)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    pub fn bind_plugin(
        &mut self,
        plugin: &dyn kelir_interface::plugin_model::AnyPlugin,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        if self.engine.is_sealed() {
            return Err(kelir_interface::plugin_model::PluginError::Halt(
                "pipeline already sealed".into(),
            ));
        }
        if !plugin.is_enabled() {
            return Ok(());
        }
        self.engine.set_active_scope(plugin.default_scope());
        plugin.register_interceptors(&mut self.engine);
        self.engine
            .set_active_scope(kelir_interface::plugin_model::MountScope::default());
        Ok(())
    }

    // TODO (L3): Session::boot calls register_storage_lock_broker before bind_plugin so the 2PC coordinator resolves the host broker.
    pub fn register_storage_lock_broker(
        &mut self,
        interposer: Arc<dyn kelir_interface::plugin_model::FsInterposer>,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        if self.engine.is_sealed() {
            return Err(kelir_interface::plugin_model::PluginError::Halt(
                "pipeline already sealed".into(),
            ));
        }
        // see: cans/05-storage-plane.md#Host-inode-lock-broker
        self.storage.set_lock_namespace(self.incarnation_id);
        let coordination = self.storage.lock_broker_coordination(interposer);
        self.engine.register_lock_broker(coordination);
        Ok(())
    }

    pub fn seal_pipeline(&mut self) -> Result<(), kelir_interface::plugin_model::PluginError> {
        self.engine.seal()
    }

    pub fn bus(&mut self) -> &mut crate::bus::BusDispatcher {
        self.owned_bus.dispatcher()
    }

    pub fn uart_ring(&self) -> &crate::uart_terminal::UartRingBuffer {
        &self.uart
    }

    pub fn record_early_boot_output(
        &mut self,
        bytes: &[u8],
    ) -> Option<kelir_interface::telemetry::ExitKind> {
        if self.uart.len().saturating_add(bytes.len()) > crate::uart_terminal::UART_RING_CAPACITY {
            self.metrics.record_drop();
        }
        self.uart.write(bytes);
        self.uart.decode_exit_kind()
    }

    pub fn early_boot_exit(&self) -> Option<kelir_interface::telemetry::ExitKind> {
        self.uart.decode_exit_kind()
    }

    pub const fn incarnation_id(&self) -> [u8; 16] {
        self.incarnation_id
    }

    // see: cans/08-lifecycle.md#CoW-fork
    pub fn lineage(&self, root_pid: u32) -> crate::planes::checkpoint::PidLineage {
        crate::planes::checkpoint::PidLineage::new_in_session(root_pid, self.incarnation_id)
    }

    pub fn fork(&self) -> Result<Self, kelir_interface::plugin_model::PluginError> {
        // TODO (L3): fork driver, clone planes, mint a new incarnation_id for the child.
        let _ = (&self.driver, &self.owned_bus, &self.incarnation_id);
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): Session::fork".into(),
        ))
    }

    // see: cans/08-lifecycle.md#Rollback
    pub async fn rollback(
        &mut self,
        lineage: crate::planes::checkpoint::PidLineage,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        let node_id = self.checkpoint.head(lineage).ok_or_else(|| {
            kelir_interface::plugin_model::PluginError::Halt(
                "no checkpoint head for PID lineage".into(),
            )
        })?;
        let snapshot = self.checkpoint.restore(node_id)?;
        self.rollback_to(&snapshot).await
    }

    // see: cans/08-lifecycle.md#Rollback
    // see: cans/plugins/vault.md#Revocation
    pub async fn rollback_to(
        &mut self,
        snapshot: &kelir_interface::driver_interface::SnapshotDescriptor,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // see: cans/08-lifecycle.md#Rollback
        if let Some(revoker) = self.engine.token_revoker() {
            revoker
                .revoke_session_tokens(&encode_hex(&self.incarnation_id))
                .await?;
        }
        let Some(driver) = self.driver.as_ref() else {
            return Err(kelir_interface::plugin_model::PluginError::Halt(
                "driver not loaded: hardware rollback needs a booted VMM (L3)".into(),
            ));
        };
        self.wire.quiesce()?;
        if let Some(ingress) = self.engine.parked_connection_reset() {
            let reset = ingress.reset_parked_connections()?;
            // TODO (L3): emit TCP RST for each reset connection id once the host proxy owns the parked sockets per cans/plugins/wire-ingress.md.
            let _ = reset;
        }
        self.storage.discard()?;
        // TODO (L3): reset session-local RNG position and virtual clock position to the snapshot.
        driver
            .lock()
            .await
            .rollback(snapshot)
            .await
            .map_err(|error| {
                kelir_interface::plugin_model::PluginError::Halt(format!(
                    "driver rollback: {error}"
                ))
            })?;
        Ok(())
    }

    // see: cans/08-lifecycle.md#Lid-sleep/wake-clock-snapping
    pub fn dispatch_clock_resync(
        &mut self,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3): subscribe IOPMrootDomain (macOS) / systemd-logind (Linux) host power events and invoke this on sleep/wake.
        let wall_clock_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos() as u64)
            .unwrap_or(0);
        self.owned_bus.dispatcher().send_control_msg(
            kelir_interface::protocol_wire::MsgType::VmLifecycleControl,
            &wall_clock_ns.to_le_bytes(),
        )
    }

    // see: cans/08-lifecycle.md#State-machine
    pub fn lifecycle_state(&self) -> LifecycleState {
        self.state
    }

    // see: cans/08-lifecycle.md#State-machine
    fn advance_lifecycle(
        &mut self,
        next: LifecycleState,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        self.state = self.state.transition(next).map_err(|error| {
            kelir_interface::plugin_model::PluginError::Halt(format!("{error:?}"))
        })?;
        Ok(())
    }

    pub fn suspend(
        &mut self,
        reason: kelir_interface::driver_interface::SuspendReason,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3): suspend the driver to disk per reason.
        let _ = reason;
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): Session::suspend".into(),
        ))
    }

    pub fn resume(&mut self) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3): resume the driver from the suspended image.
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): Session::resume".into(),
        ))
    }

    pub fn drain(
        &mut self,
        timeout_ms: u64,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3): drain in-flight frames until quiescent or timeout_ms elapses.
        let _ = timeout_ms;
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): Session::drain".into(),
        ))
    }

    pub fn stop(&mut self) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3): graceful driver terminate after drain.
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): Session::stop".into(),
        ))
    }

    // see: cans/08-lifecycle.md#Pre-teardown-flush
    pub fn stop_with_timeout(
        &mut self,
        timeout: std::time::Duration,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3): graceful driver terminate after MSG_SHUTDOWN_ACK or timeout expiry.
        let _ = timeout;
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): Session::stop_with_timeout".into(),
        ))
    }

    pub fn kill(&mut self) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3): hard driver terminate without drain.
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): Session::kill".into(),
        ))
    }

    // see: cans/09-snapshots.md#Archives-and-portability
    pub fn export_snapshot(
        &mut self,
    ) -> Result<
        Box<dyn tokio::io::AsyncRead + Send + Sync + Unpin>,
        kelir_interface::plugin_model::PluginError,
    > {
        // TODO (L3): stream the .kelir-snap archive from the driver.
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): Session::export_snapshot".into(),
        ))
    }
}
