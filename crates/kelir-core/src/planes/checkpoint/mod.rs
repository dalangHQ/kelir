// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-core::checkpoint)
// see: cans/08-lifecycle.md#Delta-chain-compaction
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

pub struct StepSlot {
    slot_id: u64,
    base_descriptor: kelir_interface::driver::SnapshotDescriptor,
    pending_delta: Option<kelir_interface::telemetry::CausalDelta>,
}

pub struct CheckpointPlane {
    active_step_slot: Arc<Mutex<Option<StepSlot>>>,
    // TODO: commit_step appends the committed step digest here (L2; SnapshotDescriptor carries no digest yet).
    #[allow(dead_code)]
    manifest_chain: Arc<RwLock<Vec<[u8; 32]>>>,
    next_slot_id: AtomicU64,
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
            next_slot_id: AtomicU64::new(0),
        }
    }

    pub fn create_step(
        &self,
        base: kelir_interface::driver::SnapshotDescriptor,
    ) -> Result<(), kelir_interface::errors::PluginError> {
        let mut slot = self.active_step_slot.lock().map_err(|_| {
            kelir_interface::errors::PluginError::Halt("step slot lock poisoned".into())
        })?;
        if slot.is_some() {
            return Err(kelir_interface::errors::PluginError::Halt(
                "step already active".into(),
            ));
        }
        let slot_id = self.next_slot_id.fetch_add(1, Ordering::SeqCst);
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
