// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-core::checkpoint)
// see: cans/08-lifecycle.md#Delta-chain-compaction
use std::sync::{Arc, Mutex, RwLock};

pub struct StepSlot {
    slot_id: u64,
    base_descriptor: kelir_interface::driver::SnapshotDescriptor,
    pending_delta: Option<kelir_interface::telemetry::CausalDelta>,
}

pub struct CheckpointPlane {
    active_step_slot: Arc<Mutex<Option<StepSlot>>>,
    manifest_chain: Arc<RwLock<Vec<[u8; 32]>>>,
}

impl Default for CheckpointPlane {
    fn default() -> Self {
        Self::new()
    }
}

impl CheckpointPlane {
    pub fn new() -> Self {
        Self {
            active_step_slot: Arc::new(Mutex::new(None)),
            manifest_chain: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn create_step(
        &self,
        base: kelir_interface::driver::SnapshotDescriptor,
    ) -> Result<(), kelir_interface::errors::PluginError> {
        let slot_id = self
            .manifest_chain
            .read()
            .map_err(|_| {
                kelir_interface::errors::PluginError::Halt("manifest chain lock poisoned".into())
            })?
            .len();
        let slot_id = u64::try_from(slot_id).map_err(|_| {
            kelir_interface::errors::PluginError::Halt("step slot id overflow".into())
        })?;
        let mut slot = self.active_step_slot.lock().map_err(|_| {
            kelir_interface::errors::PluginError::Halt("step slot lock poisoned".into())
        })?;
        *slot = Some(StepSlot {
            slot_id,
            base_descriptor: base,
            pending_delta: None,
        });
        Ok(())
    }

    pub fn commit_step(&self) -> Result<(), kelir_interface::errors::PluginError> {
        let mut slot = self.active_step_slot.lock().map_err(|_| {
            kelir_interface::errors::PluginError::Halt("step slot lock poisoned".into())
        })?;
        match slot.take() {
            Some(step) => {
                // TODO: append the committed step digest (slot_id + pending_delta) to manifest_chain.
                let _ = (step.slot_id, step.pending_delta);
                Ok(())
            }
            None => Err(kelir_interface::errors::PluginError::Halt(
                "no active step slot".into(),
            )),
        }
    }

    pub fn rollback_step(
        &self,
    ) -> Result<kelir_interface::driver::SnapshotDescriptor, kelir_interface::errors::PluginError>
    {
        let mut slot = self.active_step_slot.lock().map_err(|_| {
            kelir_interface::errors::PluginError::Halt("step slot lock poisoned".into())
        })?;
        match slot.take() {
            Some(step) => Ok(step.base_descriptor),
            None => Err(kelir_interface::errors::PluginError::Halt(
                "no active step slot".into(),
            )),
        }
    }
}
