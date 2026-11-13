// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
// see: cans/08-lifecycle.md#Delta-chain-compaction
// see: cans/09-snapshots.md#Restore-modes
// see: cans/09-snapshots.md#Groups-and-lineage
use std::sync::atomic::{AtomicU64, Ordering};

use kelir_interface::checkpoint::{CheckpointNode, ManifestGraph, PidLineage};
use kelir_interface::driver_interface::SnapshotDescriptor;
use kelir_interface::plugin_model::PluginError;

pub(crate) fn compact(
    graph: &mut ManifestGraph,
    next_slot_id: &AtomicU64,
    lineage: PidLineage,
) -> Result<u64, PluginError> {
    let head = graph
        .head(lineage)
        .ok_or_else(|| PluginError::Halt("no checkpoint head for PID lineage".into()))?;
    let chain = graph.lineage(head);
    let Some(base) = chain.last().copied() else {
        return Err(PluginError::Halt(
            "checkpoint chain has no ancestor base".into(),
        ));
    };
    let head_node = graph
        .get(head)
        .cloned()
        .ok_or_else(|| PluginError::Halt("checkpoint head missing from manifest graph".into()))?;
    let compacted_id = next_slot_id.fetch_add(1, Ordering::SeqCst);
    // TODO (L3) [lc_20_portable_snapshot_archive_export]: single-pass block copy flattens ancestor delta pages into the base sparse extents on export; CAS/BLAKE3 manifest addressing replaces sequential node ids; slop: export Ok without trailer BLAKE3 digest, manifest unchecked
    graph.insert(CheckpointNode {
        node_id: compacted_id,
        parent: Some(base),
        descriptor: head_node.descriptor,
        delta: head_node.delta,
    });
    graph.set_head(lineage, compacted_id);
    Ok(compacted_id)
}

pub(crate) fn restore(
    graph: &ManifestGraph,
    node_id: u64,
    guest_monotonic_ns: u64,
    watermark_ns: &AtomicU64,
) -> Result<SnapshotDescriptor, PluginError> {
    // TODO (L3) [lc_11_lid_sleep_clock_resync_snap]: read the live guest CLOCK_MONOTONIC through the driver surface; the watermark is the in-tree proxy; slop: wake Ok without guest clock snap to host wall clock
    let Some(node) = graph.get(node_id) else {
        return Err(PluginError::Halt(
            "checkpoint node not in manifest graph".into(),
        ));
    };
    let mut descriptor = node.descriptor.clone();
    if let Some(clock) = descriptor.guest_clock.as_mut() {
        let current = watermark_ns.load(Ordering::Acquire).max(guest_monotonic_ns);
        let clamped = clock.monotonic_offset_ns.max(current);
        clock.monotonic_offset_ns = clamped;
        watermark_ns.store(clamped, Ordering::Release);
    }
    Ok(descriptor)
}
