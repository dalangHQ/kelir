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
        // TODO: load native driver, connect bus transports, build planes, mint incarnation_id.
        let _ = config;
        Err(kelir_interface::errors::PluginError::Halt(
            "TODO: Session::boot".into(),
        ))
    }

    pub fn fork(&self) -> Result<Self, kelir_interface::errors::PluginError> {
        // TODO: fork driver, clone planes, mint a new incarnation_id for the child.
        let _ = (&self.driver, &self.bus, &self.incarnation_id);
        Err(kelir_interface::errors::PluginError::Halt(
            "TODO: Session::fork".into(),
        ))
    }

    pub fn rollback(&mut self) -> Result<(), kelir_interface::errors::PluginError> {
        // TODO: roll driver back to the active step slot and drop the storage upper file.
        Ok(())
    }

    pub fn suspend(
        &mut self,
        reason: kelir_interface::driver::SuspendReason,
    ) -> Result<(), kelir_interface::errors::PluginError> {
        // TODO: suspend the driver to disk per reason (L2: map SuspendReason to driver suspend).
        let _ = reason;
        Ok(())
    }

    pub fn resume(&mut self) -> Result<(), kelir_interface::errors::PluginError> {
        // TODO: resume the driver from the suspended image.
        Ok(())
    }

    pub fn drain(&mut self, timeout_ms: u64) -> Result<(), kelir_interface::errors::PluginError> {
        // TODO: drain in-flight frames until quiescent or timeout_ms elapses.
        let _ = timeout_ms;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), kelir_interface::errors::PluginError> {
        // TODO: graceful driver terminate after drain.
        Ok(())
    }

    pub fn kill(&mut self) -> Result<(), kelir_interface::errors::PluginError> {
        // TODO: hard driver terminate without drain.
        Ok(())
    }
}
