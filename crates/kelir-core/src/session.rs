// see: cans/08-lifecycle.md#Session-and-context-handles
// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
// see: cans/14-observability.md#Metrics-pipeline
use std::sync::Arc;

use kelir_interface::plugin::PipelineBinder;
pub struct Session {
    pub terminal: crate::planes::terminal::TerminalPlane,
    pub storage: crate::planes::storage::StoragePlane,
    pub wire: crate::planes::wire::WirePlane,
    pub checkpoint: crate::planes::checkpoint::CheckpointPlane,
    pub host: crate::planes::host::HostPlane,
    metrics: std::sync::Arc<kelir_otlp::registry::MetricRegistry>,
    uart: crate::uart::UartRingBuffer,
    driver: Box<dyn kelir_interface::driver::VmmDriver + Send + Sync>,
    owned_bus: crate::bus::OwnedBusDispatcher,
    engine: crate::dispatch::PipelineEngine,
    incarnation_id: [u8; 16],
    plugins: Vec<crate::plugin_registry::PluginRegistration>,
}

impl Session {
    pub fn boot(
        config: kelir_interface::driver::VmConfig,
    ) -> Result<Self, kelir_interface::plugin::PluginError> {
        Self::boot_with_plugins(config, &[])
    }

    pub fn boot_with_plugins(
        config: kelir_interface::driver::VmConfig,
        plugins: &[crate::plugin_registry::PluginRegistration],
    ) -> Result<Self, kelir_interface::plugin::PluginError> {
        Self::boot_with_engine(
            config,
            plugins,
            crate::dispatch::PipelineEngine::new(),
            std::sync::Arc::new(kelir_otlp::registry::MetricRegistry::new()),
            Vec::new(),
        )
    }

    pub fn boot_with_engine(
        config: kelir_interface::driver::VmConfig,
        plugins: &[crate::plugin_registry::PluginRegistration],
        engine: crate::dispatch::PipelineEngine,
        metrics: std::sync::Arc<kelir_otlp::registry::MetricRegistry>,
        synthetic_files: Vec<crate::planes::storage::synthetic::SyntheticFile>,
    ) -> Result<Self, kelir_interface::plugin::PluginError> {
        let _ = crate::plugin_registry::resolve_plugin_registrations(plugins)?;
        // TODO (L3): load native driver, connect the control and mux SPSC rings, build planes, mint incarnation_id.
        metrics.set_active_vcpus(u32::from(config.vcpus));
        let _ = config;
        let _ = engine;
        let _ = metrics;
        let _ = synthetic_files;
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO (L3): Session::boot".into(),
        ))
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
        driver: Box<dyn kelir_interface::driver::VmmDriver + Send + Sync>,
        owned_bus: crate::bus::OwnedBusDispatcher,
        engine: crate::dispatch::PipelineEngine,
        incarnation_id: [u8; 16],
        plugins: Vec<crate::plugin_registry::PluginRegistration>,
        metrics: std::sync::Arc<kelir_otlp::registry::MetricRegistry>,
    ) -> Self {
        Self {
            terminal: terminal.with_metrics(metrics.clone()),
            storage: storage.with_metrics(metrics.clone()),
            wire: wire.with_metrics(metrics.clone()),
            checkpoint: checkpoint.with_metrics(metrics.clone()),
            host: host.with_metrics(metrics.clone()),
            metrics: metrics.clone(),
            uart: crate::uart::UartRingBuffer::new(),
            driver,
            owned_bus,
            engine: engine.with_metrics(metrics),
            incarnation_id,
            plugins,
        }
    }

    pub fn metrics(&self) -> &std::sync::Arc<kelir_otlp::registry::MetricRegistry> {
        &self.metrics
    }

    pub fn pipeline(&self) -> &crate::dispatch::PipelineEngine {
        &self.engine
    }

    pub fn pipeline_mut(&mut self) -> &mut crate::dispatch::PipelineEngine {
        &mut self.engine
    }

    pub fn bind_plugin(
        &mut self,
        plugin: &dyn kelir_interface::plugin::AnyPlugin,
    ) -> Result<(), kelir_interface::plugin::PluginError> {
        if self.engine.is_sealed() {
            return Err(kelir_interface::plugin::PluginError::Halt(
                "pipeline already sealed".into(),
            ));
        }
        if !plugin.is_enabled() {
            return Ok(());
        }
        self.engine.set_active_scope(plugin.default_scope());
        plugin.register_interceptors(&mut self.engine);
        self.engine
            .set_active_scope(kelir_interface::plugin::MountScope::default());
        Ok(())
    }

    // TODO (L3): Session::boot calls register_storage_lock_broker before bind_plugin so the 2PC coordinator resolves the host broker.
    pub fn register_storage_lock_broker(
        &mut self,
        interposer: Arc<dyn kelir_interface::plugin::FsInterposer>,
    ) -> Result<(), kelir_interface::plugin::PluginError> {
        if self.engine.is_sealed() {
            return Err(kelir_interface::plugin::PluginError::Halt(
                "pipeline already sealed".into(),
            ));
        }
        let coordination = self.storage.lock_broker_coordination(interposer);
        self.engine.register_lock_broker(coordination);
        Ok(())
    }

    pub fn seal_pipeline(&mut self) -> Result<(), kelir_interface::plugin::PluginError> {
        self.engine.seal()
    }

    pub fn bus(&mut self) -> &mut crate::bus::BusDispatcher {
        self.owned_bus.dispatcher()
    }

    pub fn uart_ring(&self) -> &crate::uart::UartRingBuffer {
        &self.uart
    }

    pub fn record_early_boot_output(
        &mut self,
        bytes: &[u8],
    ) -> Option<kelir_interface::telemetry::ExitKind> {
        if self.uart.len().saturating_add(bytes.len()) > crate::uart::UART_RING_CAPACITY {
            self.metrics.record_drop();
        }
        self.uart.write(bytes);
        self.uart.decode_exit_kind()
    }

    pub fn early_boot_exit(&self) -> Option<kelir_interface::telemetry::ExitKind> {
        self.uart.decode_exit_kind()
    }

    pub fn fork(&self) -> Result<Self, kelir_interface::plugin::PluginError> {
        // TODO (L3): fork driver, clone planes, mint a new incarnation_id for the child.
        let _ = (&self.driver, &self.owned_bus, &self.incarnation_id);
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO (L3): Session::fork".into(),
        ))
    }

    pub fn rollback(&mut self) -> Result<(), kelir_interface::plugin::PluginError> {
        // TODO (L3): roll driver back to the active checkpoint branch head and drop the storage upper file.
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO (L3): Session::rollback".into(),
        ))
    }

    pub fn suspend(
        &mut self,
        reason: kelir_interface::driver::SuspendReason,
    ) -> Result<(), kelir_interface::plugin::PluginError> {
        // TODO (L3): suspend the driver to disk per reason.
        let _ = reason;
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO (L3): Session::suspend".into(),
        ))
    }

    pub fn resume(&mut self) -> Result<(), kelir_interface::plugin::PluginError> {
        // TODO (L3): resume the driver from the suspended image.
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO (L3): Session::resume".into(),
        ))
    }

    pub fn drain(&mut self, timeout_ms: u64) -> Result<(), kelir_interface::plugin::PluginError> {
        // TODO (L3): drain in-flight frames until quiescent or timeout_ms elapses.
        let _ = timeout_ms;
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO (L3): Session::drain".into(),
        ))
    }

    pub fn stop(&mut self) -> Result<(), kelir_interface::plugin::PluginError> {
        // TODO (L3): graceful driver terminate after drain.
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO (L3): Session::stop".into(),
        ))
    }

    pub fn kill(&mut self) -> Result<(), kelir_interface::plugin::PluginError> {
        // TODO (L3): hard driver terminate without drain.
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO (L3): Session::kill".into(),
        ))
    }
}
