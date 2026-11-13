// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
// see: cans/10-attestation.md#Step-evaluation-loop
// see: cans/08-lifecycle.md#Delta-chain-compaction
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;

use kelir_interface::checkpoint::{CheckpointNode, ManifestGraph, PidLineage, StepSlot};
use kelir_interface::driver_interface::SnapshotDescriptor;
use kelir_interface::plugin_model::PluginError;

pub(crate) fn create_step(
    steps: &mut HashMap<PidLineage, StepSlot>,
    next_slot_id: &AtomicU64,
    lineage: PidLineage,
    base: SnapshotDescriptor,
) -> Result<u64, PluginError> {
    // TODO (L3) [lc_03_step_evaluation_taint_rollback]: invoke this create from the trap-match pre-hook so active_step() has a live slot per trap event; slop: trap without live step slot, evaluation unattributed
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
    graph: &mut ManifestGraph,
    step: StepSlot,
    lineage: PidLineage,
) -> Result<u64, PluginError> {
    let parent = graph.head(lineage);
    graph.insert(CheckpointNode {
        node_id: step.slot_id(),
        parent,
        descriptor: step.base_descriptor().clone(),
        delta: step.pending_delta().cloned(),
    });
    graph.set_head(lineage, step.slot_id());
    // TODO (L3) [lifecycle_checkpoint]: flush the staging delta into the Base sparse extents on the checkpoint slot; slop: staging delta never flushed, base extents stale
    Ok(step.slot_id())
}

pub(crate) fn rollback_step(
    steps: &mut HashMap<PidLineage, StepSlot>,
    lineage: PidLineage,
) -> Result<SnapshotDescriptor, PluginError> {
    match steps.remove(&lineage) {
        Some(step) => {
            // TODO (L3) [lc_03_step_evaluation_taint_rollback]: discard the working delta from the checkpoint slot in <8ms; slop: delta kept past 8ms, rollback leaves dirty pages
            Ok(step.base_descriptor().clone())
        }
        None => Err(PluginError::Halt(
            "no active step slot for PID lineage".into(),
        )),
    }
}

pub(crate) fn branch_from(
    graph: &mut ManifestGraph,
    node_id: u64,
    lineage: PidLineage,
) -> Result<(), PluginError> {
    if graph.get(node_id).is_none() {
        return Err(PluginError::Halt(
            "branch source node not in manifest graph".into(),
        ));
    }
    if graph.has_lineage(lineage) {
        return Err(PluginError::Halt(
            "PID lineage already has a chain head".into(),
        ));
    }
    graph.set_head(lineage, node_id);
    Ok(())
}
