// see: cans/08-lifecycle.md#Session-and-context-handles
// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
use std::sync::{Arc, Mutex};

pub struct Session {
    pub terminal: crate::planes::terminal::TerminalPlane,
    pub storage: crate::planes::storage::StoragePlane,
    pub wire: crate::planes::wire::WirePlane,
    pub checkpoint: crate::planes::checkpoint::CheckpointPlane,
    pub host: crate::planes::host::HostPlane,
    driver: Arc<Mutex<Box<dyn kelir_interface::driver::VmmDriver + Send + Sync>>>,
    bus: Arc<Mutex<crate::bus::BusDispatcher>>,
    incarnation_id: [u8; 16],
}

impl Session {
    pub fn boot(
        config: kelir_interface::driver::VmConfig,
    ) -> Result<Self, kelir_interface::errors::PluginError> {
        // see: cans/08-lifecycle.md#Session-and-context-handles
        let _ = config;
        Err(kelir_interface::errors::PluginError::Halt(
            "TODO: Session::boot".into(),
        ))
    }

    // see: cans/08-lifecycle.md#Session-and-context-handles
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        terminal: crate::planes::terminal::TerminalPlane,
        storage: crate::planes::storage::StoragePlane,
        wire: crate::planes::wire::WirePlane,
        checkpoint: crate::planes::checkpoint::CheckpointPlane,
        host: crate::planes::host::HostPlane,
        driver: Arc<Mutex<Box<dyn kelir_interface::driver::VmmDriver + Send + Sync>>>,
        bus: Arc<Mutex<crate::bus::BusDispatcher>>,
        incarnation_id: [u8; 16],
    ) -> Self {
        Self {
            terminal,
            storage,
            wire,
            checkpoint,
            host,
            driver,
            bus,
            incarnation_id,
        }
    }

    pub fn fork(&self) -> Result<Self, kelir_interface::errors::PluginError> {
        // see: cans/08-lifecycle.md#Session-and-context-handles
        let _ = (&self.driver, &self.bus, &self.incarnation_id);
        Err(kelir_interface::errors::PluginError::Halt(
            "TODO: Session::fork".into(),
        ))
    }

    pub fn rollback(&mut self) -> Result<(), kelir_interface::errors::PluginError> {
        // see: cans/08-lifecycle.md#Session-and-context-handles
        Ok(())
    }

    pub fn suspend(
        &mut self,
        reason: kelir_interface::driver::SuspendReason,
    ) -> Result<(), kelir_interface::errors::PluginError> {
        // see: cans/08-lifecycle.md#Session-and-context-handles
        let _ = reason;
        Ok(())
    }

    pub fn resume(&mut self) -> Result<(), kelir_interface::errors::PluginError> {
        // see: cans/08-lifecycle.md#Session-and-context-handles
        Ok(())
    }

    pub fn drain(&mut self, timeout_ms: u64) -> Result<(), kelir_interface::errors::PluginError> {
        // see: cans/08-lifecycle.md#Session-and-context-handles
        let _ = timeout_ms;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), kelir_interface::errors::PluginError> {
        // see: cans/08-lifecycle.md#Session-and-context-handles
        Ok(())
    }

    pub fn kill(&mut self) -> Result<(), kelir_interface::errors::PluginError> {
        // see: cans/08-lifecycle.md#Session-and-context-handles
        Ok(())
    }
}
