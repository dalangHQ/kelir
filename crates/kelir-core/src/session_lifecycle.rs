// see: cans/08-lifecycle.md#Session-and-context-handles
// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
// see: cans/14-observability.md#Metrics-pipeline
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use kelir_interface::driver_interface::{
    DriverCapabilities, DriverError, EntropyBackend, EntropyStreamPosition, VmConfig, VmmDriver,
    WatchdogAction,
};
use kelir_interface::handles_lifecycle::{DriverVmControl, VmHandle};
use kelir_interface::hex_codec::encode_hex;
use kelir_interface::lifecycle_state::LifecycleState;
use kelir_interface::plugin_model::{LifecycleCtx, LifecycleEvent, PipelineBinder, VetoList};
use kelir_interface::vsock_mesh::VAULT_METADATA_VSOCK_PORT;

// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
#[derive(Debug, Clone, Default)]
pub struct PlaneWiring {
    pub terminal: Option<crate::planes::terminal::TerminalWiring>,
    pub wire: Option<crate::planes::wire::WireWiring>,
    pub storage: crate::planes::storage::StorageConfig,
}

pub struct Session {
    pub terminal: Arc<crate::planes::terminal::TerminalPlane>,
    pub storage: crate::planes::storage::StoragePlane,
    pub wire: crate::planes::wire::WirePlane,
    pub checkpoint: Arc<crate::planes::checkpoint::CheckpointPlane>,
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
    config: VmConfig,
    vetoes: VetoList,
    session_id: [u8; 16],
    incarnation_id: [u8; 16],
    plugins: Vec<crate::plugin_registry::PluginRegistration>,
    detached: bool,
    state: LifecycleState,
}

// see: cans/artifacts/code/plugin-trait.rs#LifecycleCtx
struct NullDriver;

#[async_trait]
impl VmmDriver for NullDriver {
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities {
            backend: "unbound",
            cold_boot_budget_ms: 0,
            rollback_budget_ms: 0,
            fork_budget_ms: 0,
            requires_hardware_vmm: false,
            supports_cow_fork: false,
        }
    }

    async fn boot(&mut self, _config: VmConfig) -> Result<(), DriverError> {
        Err(DriverError::Unsupported("no VMM driver bound (L2)".into()))
    }

    async fn set_entropy_backend(&mut self, _backend: EntropyBackend) -> Result<(), DriverError> {
        Err(DriverError::Unsupported("no VMM driver bound (L2)".into()))
    }

    async fn entropy_position(&self) -> Result<EntropyStreamPosition, DriverError> {
        Err(DriverError::Unsupported("no VMM driver bound (L2)".into()))
    }

    async fn restore_entropy_position(
        &mut self,
        _position: EntropyStreamPosition,
    ) -> Result<(), DriverError> {
        Err(DriverError::Unsupported("no VMM driver bound (L2)".into()))
    }

    async fn set_watchdog(
        &mut self,
        _timeout: Duration,
        _action: WatchdogAction,
    ) -> Result<(), DriverError> {
        Err(DriverError::Unsupported("no VMM driver bound (L2)".into()))
    }
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

// see: cans/01-architecture.md#Host-state-root
pub fn session_dir(
    session_id: &[u8; 16],
) -> Result<std::path::PathBuf, kelir_interface::plugin_model::PluginError> {
    let home = std::env::var("HOME").map_err(|_| {
        kelir_interface::plugin_model::PluginError::Halt(
            "HOME is not set; cannot locate ~/.kelir session root".into(),
        )
    })?;
    Ok(std::path::PathBuf::from(home)
        .join(".kelir")
        .join("sessions")
        .join(encode_hex(session_id)))
}

// see: cans/01-architecture.md#Host-state-root
// TODO (L3): the runtime child janitor sweeps orphaned session dirs on startup.
pub fn reap_ephemeral_session_dir(
    session_id: &[u8; 16],
) -> Result<(), kelir_interface::plugin_model::PluginError> {
    let dir = session_dir(session_id)?;
    match std::fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(kelir_interface::plugin_model::PluginError::Halt(format!(
            "reap session dir: {error}"
        ))),
    }
}

impl Session {
    // see: cans/08-lifecycle.md#Detached-lifecycle
    pub fn relay_socket_path(
        session_id: &[u8; 16],
    ) -> Result<std::path::PathBuf, kelir_interface::plugin_model::PluginError> {
        Ok(session_dir(session_id)?.join("relay.sock"))
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

    pub fn set_vetoes(&mut self, vetoes: VetoList) {
        self.vetoes = vetoes;
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
        Self::boot_with_plugins(config, &[], None, Vec::new(), PlaneWiring::default())
    }

    // see: cans/02-plugin-model.md#Foreign-plugin-proxy
    pub fn boot_with_plugins(
        config: kelir_interface::driver_interface::VmConfig,
        plugins: &[crate::plugin_registry::PluginRegistration],
        trampoline: Option<&kelir_interface::fusion_plugin::FusedTrampolineDescriptor>,
        synthetic_files: Vec<crate::planes::storage::storage_synthetic::SyntheticFile>,
        plane_wiring: PlaneWiring,
    ) -> Result<Self, kelir_interface::plugin_model::PluginError> {
        let mut engine = crate::dispatch_plugin::PipelineEngine::new();
        if let Some(descriptor) = trampoline {
            engine = engine.with_trampoline(descriptor.clone());
        }
        let mut session = Self::boot_with_engine(
            config,
            plugins,
            engine,
            std::sync::Arc::new(kelir_otlp::registry_otlp::MetricRegistry::new()),
            synthetic_files,
            plane_wiring,
        )?;
        session.advance_lifecycle(LifecycleState::Running)?;
        Ok(session)
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
        // see: cans/01-architecture.md#Host-state-root
        let workspace_root = session_dir(&session_id)?.join("workspace");
        let storage = crate::planes::storage::StoragePlane::new(
            workspace_root.clone(),
            workspace_root.join("upper.img"),
            DEFAULT_WORKSPACE_QUOTA_BYTES,
            DEFAULT_WORKSPACE_QUOTA_INODES,
        )
        .with_synthetic_files(synthetic_files)
        .with_wiring(plane_wiring.storage);
        // TODO (L3): anchor derives broker_dir from the live host broker journal once the broker owns its layout.
        // see: cans/05-storage-plane.md#Workspace-commit
        storage.reconciler(
            workspace_root.join("broker"),
            crate::planes::storage::storage_reconciler::TraversalGuard::StrictResolveBeneath,
        );
        // see: cans/plugins/replay.md#Invariants-and-policy-(replay)
        let wire = crate::planes::wire::WirePlane::new().with_wiring(match plane_wiring.wire {
            Some(mut wiring) => {
                wiring.sinkhole_drop_all |= config.network_off;
                wiring
            }
            None => crate::planes::wire::WireWiring {
                sinkhole_drop_all: config.network_off,
                ..Default::default()
            },
        });
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
        let session_checkpoint_plane = Arc::new(
            crate::planes::checkpoint::CheckpointPlane::new().with_metrics(metrics.clone()),
        );
        let engine = engine
            .with_metrics(metrics.clone())
            .with_terminal(Arc::clone(&terminal))
            .with_checkpoint(Arc::clone(&session_checkpoint_plane));
        metrics.set_active_vcpus(u32::from(config.vcpus));
        // TODO (L3): DriverRouter probes KVM/HVF/AVF and completes the driver boot handshake; vm_handle stays unbound until a backend loads.
        let driver = match crate::vmm::DriverRouter::load_native_driver(
            config.driver_policy.driver.as_deref(),
            matches!(
                config.driver_policy.wx_enforcement,
                kelir_interface::driver_interface::WxPolicy::Strict
            ),
            config.driver_policy.allow_emulated_fallback,
        ) {
            Ok(loaded) => Some(Arc::new(tokio::sync::Mutex::new(loaded))),
            Err(DriverError::Unsupported(_)) => {
                // see: cans/01-architecture.md#Page-size-dynamic-boot-selection
                // TODO (L3): consumed by driver-rustvmm/hvf kernel-fixture page-size selection.
                if config.page_size_kb != 4 {
                    tracing::warn!(
                        "page_size_kb={} set but the driver backend returned Unsupported; unconsumed (L3)",
                        config.page_size_kb
                    );
                }
                // see: cans/08-lifecycle.md#Clock-invariants
                // TODO (L3): consumed by driver-rustvmm kvm-clock boot-param injection.
                if let Some(clocksource) = config.pinned_clocksource.as_deref() {
                    tracing::warn!(
                        "pinned_clocksource={clocksource} set but the driver backend returned Unsupported; unconsumed (L3)"
                    );
                }
                // see: cans/08-lifecycle.md#Host-crash-reaper
                // TODO (L3): consumed by the driver host-crash reaper preemption window.
                if config.preemption_grace_ms != 0 {
                    tracing::warn!(
                        "preemption_grace_ms={} set but the driver backend returned Unsupported; unconsumed (L3)",
                        config.preemption_grace_ms
                    );
                }
                None
            }
            Err(_) => None,
        };
        let session = Self {
            terminal,
            storage: storage.with_metrics(metrics.clone()),
            wire: wire.with_metrics(metrics.clone()),
            checkpoint: session_checkpoint_plane,
            host: crate::planes::host::HostPlane::new().with_metrics(metrics.clone()),
            metrics,
            uart: crate::uart_terminal::UartRingBuffer::new(),
            driver,
            owned_bus: crate::bus::OwnedBusDispatcher::new(
                crate::bus::CONTROL_RING_CAPACITY,
                crate::bus::MUX_RING_CAPACITY,
            ),
            engine,
            config,
            vetoes: Vec::new(),
            session_id,
            incarnation_id,
            plugins: resolved,
            detached: false,
            state: LifecycleState::Initializing,
        };
        Ok(session)
    }

    // see: cans/08-lifecycle.md#State-machine
    pub fn begin_running(&mut self) -> Result<(), kelir_interface::plugin_model::PluginError> {
        self.advance_lifecycle(LifecycleState::Running)
    }

    pub fn plugins(&self) -> &[crate::plugin_registry::PluginRegistration] {
        &self.plugins
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

    // see: cans/03-sdk-architecture.md#Session-surfaces
    pub fn vm(&self) -> VmHandle {
        self.vm_handle()
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

    // see: cans/01-architecture.md#Early-boot-diagnostics
    pub fn record_early_boot_output(&mut self, bytes: &[u8]) {
        if self.uart.len().saturating_add(bytes.len()) > crate::uart_terminal::UART_RING_CAPACITY {
            self.metrics.record_drop();
        }
        self.uart.write(bytes);
    }

    // see: cans/01-architecture.md#Early-boot-diagnostics
    // TODO (L3): the supervisor scrapes only when the boot budget expires without VmReady or on hypervisor fault transitions.
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

    // see: cans/08-lifecycle.md#CoW-fork
    // SAFETY: the wrapper copies every parent field it needs before the first await so the returned future never borrows the !Sync parent.
    pub fn fork(
        &self,
    ) -> impl Future<Output = Result<Self, kelir_interface::plugin_model::PluginError>> + Send {
        // TODO (L3): backend drivers implement the native CoW clone behind VmmDriver::fork.
        // TODO (L3): rebind vsock channels and wire wiring onto the child engine; the SDK layer rebinds the plugin graph via fork_state.
        let incarnation_id = mint_uuid_v7();
        // see: cans/08-lifecycle.md#CoW-fork
        let parent_driver = self.driver.clone();
        let metrics = Arc::clone(&self.metrics);
        let terminal = Arc::clone(&self.terminal);
        let storage_wiring = self.storage.wiring().clone();
        let config = self.config.clone();
        let session_id = self.session_id;
        let plugins = self.plugins.clone();
        let detached = self.detached;
        let state = self.state;
        async move {
            // see: cans/08-lifecycle.md#CoW-fork
            let child_driver = match parent_driver {
                Some(driver) => {
                    let forked: Box<dyn VmmDriver + Send + Sync> =
                        driver.lock().await.fork().await.map_err(|error| {
                            kelir_interface::plugin_model::PluginError::Halt(format!(
                                "driver fork: {error}"
                            ))
                        })?;
                    Some(Arc::new(tokio::sync::Mutex::new(forked)))
                }
                None => None,
            };
            let workspace_root = session_dir(&session_id)?.join("workspace");
            let storage = crate::planes::storage::StoragePlane::new(
                workspace_root.clone(),
                workspace_root.join("upper.img"),
                DEFAULT_WORKSPACE_QUOTA_BYTES,
                DEFAULT_WORKSPACE_QUOTA_INODES,
            )
            .with_wiring(storage_wiring);
            // TODO (L3): anchor derives broker_dir from the live host broker journal once the broker owns its layout.
            // see: cans/05-storage-plane.md#Workspace-commit
            storage.reconciler(
                workspace_root.join("broker"),
                crate::planes::storage::storage_reconciler::TraversalGuard::StrictResolveBeneath,
            );
            let checkpoint = Arc::new(
                crate::planes::checkpoint::CheckpointPlane::new()
                    .with_metrics(Arc::clone(&metrics)),
            );
            let engine = crate::dispatch_plugin::PipelineEngine::new()
                .with_metrics(Arc::clone(&metrics))
                .with_terminal(Arc::clone(&terminal))
                .with_checkpoint(Arc::clone(&checkpoint));
            Ok(Self {
                terminal,
                storage: storage.with_metrics(Arc::clone(&metrics)),
                wire: crate::planes::wire::WirePlane::new().with_metrics(Arc::clone(&metrics)),
                checkpoint,
                host: crate::planes::host::HostPlane::new().with_metrics(Arc::clone(&metrics)),
                metrics,
                uart: crate::uart_terminal::UartRingBuffer::new(),
                driver: child_driver,
                owned_bus: crate::bus::OwnedBusDispatcher::new(
                    crate::bus::CONTROL_RING_CAPACITY,
                    crate::bus::MUX_RING_CAPACITY,
                ),
                engine,
                config,
                vetoes: Vec::new(),
                session_id,
                incarnation_id,
                plugins,
                detached,
                state,
            })
        }
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
        let guest_monotonic_ns = self.vm_handle().guest_monotonic_ns();
        let snapshot = self.checkpoint.restore(node_id, guest_monotonic_ns)?;
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
                .revoke_session_tokens(&encode_hex(&self.session_id))
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

    // see: cans/08-lifecycle.md#Hypervisor-primitives
    // TODO (L3): backend drivers implement the suspend-image write; Session advances state only after driver success.
    pub async fn suspend(
        &mut self,
        reason: kelir_interface::driver_interface::SuspendReason,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        let Some(driver) = self.driver.as_ref() else {
            return Err(kelir_interface::plugin_model::PluginError::Halt(
                "driver suspend is TODO (L3): no VMM driver loaded".into(),
            ));
        };
        let path = session_dir(&self.session_id)?.join("suspend.img");
        driver
            .lock()
            .await
            .suspend(reason, &path)
            .await
            .map_err(|error| {
                kelir_interface::plugin_model::PluginError::Halt(format!(
                    "driver suspend is TODO (L3): {error}"
                ))
            })?;
        self.advance_lifecycle(LifecycleState::Suspended(reason))
    }

    // see: cans/08-lifecycle.md#Hypervisor-primitives
    // TODO (L3): backend drivers implement the suspended-image restore; Session advances state only after driver success.
    pub async fn resume(&mut self) -> Result<(), kelir_interface::plugin_model::PluginError> {
        let Some(driver) = self.driver.as_ref() else {
            return Err(kelir_interface::plugin_model::PluginError::Halt(
                "driver resume is TODO (L3): no VMM driver loaded".into(),
            ));
        };
        let path = session_dir(&self.session_id)?.join("suspend.img");
        driver.lock().await.resume(&path).await.map_err(|error| {
            kelir_interface::plugin_model::PluginError::Halt(format!(
                "driver resume is TODO (L3): {error}"
            ))
        })?;
        self.advance_lifecycle(LifecycleState::Running)
    }

    pub fn drain(
        &mut self,
        timeout_ms: u64,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3): drain in-flight frames until quiescent or timeout_ms elapses.
        let _ = timeout_ms;
        self.advance_lifecycle(LifecycleState::Draining)
    }

    // see: cans/artifacts/code/plugin-trait.rs#LifecycleCtx
    // TODO (L3): a real policy plugin arms the watchdog from on_vm_init; boot warns while none does.
    pub async fn run_lifecycle_hook(
        &mut self,
        event: LifecycleEvent,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        let session_id = encode_hex(&self.session_id);
        // see: cans/08-lifecycle.md#Watchdog
        let watchdog_probe = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        match self.driver.as_ref() {
            Some(driver) => {
                let mut guard = driver.lock().await;
                let mut ctx = LifecycleCtx::new_with_watchdog_probe(
                    &self.config,
                    &session_id,
                    &mut **guard,
                    &mut self.vetoes,
                    std::sync::Arc::clone(&watchdog_probe),
                );
                self.engine.dispatch_lifecycle(event, &mut ctx).await?;
            }
            None => {
                let mut driver = NullDriver;
                let mut ctx = LifecycleCtx::new_with_watchdog_probe(
                    &self.config,
                    &session_id,
                    &mut driver,
                    &mut self.vetoes,
                    std::sync::Arc::clone(&watchdog_probe),
                );
                self.engine.dispatch_lifecycle(event, &mut ctx).await?;
            }
        }
        if matches!(event, LifecycleEvent::VmInit)
            && self.config.enable_watchdog
            && !watchdog_probe.load(std::sync::atomic::Ordering::Acquire)
        {
            // see: cans/08-lifecycle.md#Watchdog
            tracing::warn!(
                "enable_watchdog=true finished on_vm_init without any lifecycle participant arming ctx.set_watchdog"
            );
        }
        Ok(())
    }

    // see: cans/08-lifecycle.md#State-machine
    pub async fn shutdown(&mut self) -> Result<(), kelir_interface::plugin_model::PluginError> {
        self.run_lifecycle_hook(LifecycleEvent::VmShutdown).await?;
        self.stop()
    }

    // see: cans/06-storage-catalog.md#Entity-model
    fn teardown_catalog(&self) -> Result<(), kelir_interface::plugin_model::PluginError> {
        let catalog =
            kelir_catalog::CatalogDb::open(kelir_catalog::catalog_path().map_err(|error| {
                kelir_interface::plugin_model::PluginError::Halt(format!("{error}"))
            })?)
            .map_err(|error| {
                kelir_interface::plugin_model::PluginError::Halt(format!("catalog open: {error}"))
            })?;
        let owned = match catalog.lookup_session(&self.session_id) {
            Ok(Some(row)) => {
                row.lease_kind == kelir_catalog::schema_catalog::LeaseKind::Ephemeral
                    && row.incarnation_id.0 == self.incarnation_id
                    && row.runner_pid == i64::from(std::process::id())
            }
            Ok(None) => true,
            Err(error) => {
                return Err(kelir_interface::plugin_model::PluginError::Halt(format!(
                    "catalog lookup: {error}"
                )));
            }
        };
        if !owned {
            return Ok(());
        }
        catalog
            .unregister_session(&self.session_id)
            .map_err(|error| {
                kelir_interface::plugin_model::PluginError::Halt(format!(
                    "catalog unregister: {error}"
                ))
            })?;
        reap_ephemeral_session_dir(&self.session_id)
    }

    pub fn stop(&mut self) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3): graceful driver terminate after drain.
        self.flush_workspace_before_terminate()?;
        self.advance_lifecycle(LifecycleState::Terminated)?;
        self.teardown_catalog()
    }

    // see: cans/08-lifecycle.md#Pre-teardown-flush
    // see: cans/05-storage-plane.md#Workspace-commit
    fn flush_workspace_before_terminate(
        &self,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        let workspace_root = session_dir(&self.session_id)?.join("workspace");
        let request = crate::planes::storage::storage_reconciler::FsReconcileRequest {
            cwd: workspace_root.display().to_string(),
            atomic_staging: true,
            fsync: true,
        };
        match self.storage.reconcile(request) {
            Ok(_stats) => Ok(()),
            Err(kelir_interface::plugin_model::PluginError::Halt(reason))
                if reason.contains("needs the anchored reconciler") =>
            {
                tracing::warn!(
                    "stop: storage reconciler not anchored; skipping the pre-teardown workspace flush"
                );
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    // see: cans/08-lifecycle.md#Pre-teardown-flush
    pub fn stop_with_timeout(
        &mut self,
        timeout: std::time::Duration,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3): graceful driver terminate after MSG_SHUTDOWN_ACK or timeout expiry.
        if timeout == Duration::ZERO {
            return self.kill();
        }
        let _ = timeout;
        self.flush_workspace_before_terminate()?;
        self.advance_lifecycle(LifecycleState::Terminated)?;
        self.teardown_catalog()
    }

    pub fn kill(&mut self) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3): hard driver terminate without drain or flush.
        self.advance_lifecycle(LifecycleState::Terminated)?;
        self.teardown_catalog()
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
