// see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use kelir_interface::checkpoint::{CheckpointOps, PidLineage};
use kelir_interface::driver_interface::SnapshotDescriptor;
use kelir_interface::plugin_model::PluginError;

use crate::service_step_eval::StepSlot;

pub(crate) fn create_step(
    steps: &mut HashMap<PidLineage, StepSlot>,
    next_slot_id: &AtomicU64,
    lineage: PidLineage,
    base: SnapshotDescriptor,
) -> Result<u64, PluginError> {
    // L3-GAP: [lc_03_step_evaluation_taint_rollback]: invoke this create from the trap-match pre-hook so active_step() has a live slot per trap event; slop: trap without live step slot, evaluation unattributed
    if steps.contains_key(&lineage) {
        return Err(PluginError::Halt(
            "step slot already active for PID lineage".into(),
        ));
    }
    let slot_id = next_slot_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    steps.insert(lineage, StepSlot::new(slot_id, base));
    Ok(slot_id)
}

pub(crate) fn active_descriptor(
    steps: &HashMap<PidLineage, StepSlot>,
    lineage: PidLineage,
) -> Option<SnapshotDescriptor> {
    steps
        .get(&lineage)
        .map(|slot| slot.base_descriptor().clone())
}

pub(crate) fn take_step(
    steps: &mut HashMap<PidLineage, StepSlot>,
    lineage: PidLineage,
) -> Result<StepSlot, PluginError> {
    steps
        .remove(&lineage)
        .ok_or_else(|| PluginError::Halt("no active step slot for PID lineage".into()))
}

pub(crate) fn commit_step(
    ops: &Arc<dyn CheckpointOps>,
    steps: &mut HashMap<PidLineage, StepSlot>,
    lineage: PidLineage,
) -> Result<u64, PluginError> {
    let step = take_step(steps, lineage)?;
    // L3-GAP: [lifecycle_checkpoint]: flush the staging delta into the Base sparse extents on the checkpoint slot; slop: staging delta never flushed, base extents stale
    ops.snapshot(lineage, step.base_descriptor().clone())
}

pub(crate) fn rollback_step(
    steps: &mut HashMap<PidLineage, StepSlot>,
    lineage: PidLineage,
) -> Result<SnapshotDescriptor, PluginError> {
    match steps.remove(&lineage) {
        Some(step) => {
            // L3-GAP: [lc_03_step_evaluation_taint_rollback]: discard the working delta from the checkpoint slot in <8ms; slop: delta kept past 8ms, rollback leaves dirty pages
            Ok(step.base_descriptor().clone())
        }
        None => Err(PluginError::Halt(
            "no active step slot for PID lineage".into(),
        )),
    }
}

pub(crate) fn branch_from(
    ops: &Arc<dyn CheckpointOps>,
    node_id: u64,
    lineage: PidLineage,
) -> Result<(), PluginError> {
    ops.branch_from(node_id, lineage)
}
