// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use kelir_interface::checkpoint::{CheckpointOps, ManifestGraph, PidLineage};
use kelir_interface::driver_interface::SnapshotDescriptor;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::telemetry::MetricRegistry;

// TODO (L3) [lifecycle_checkpoint]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
pub(crate) struct RealCheckpointOps {
    pub(crate) graph: RwLock<ManifestGraph>,
    pub(crate) next_node_id: AtomicU64,
    pub(crate) monotonic_watermark_ns: AtomicU64,
    pub(crate) metrics: Arc<MetricRegistry>,
}

// TODO (L3) [lifecycle_checkpoint]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
impl RealCheckpointOps {
    pub(crate) fn new(metrics: Arc<MetricRegistry>) -> Self {
        Self {
            graph: RwLock::new(ManifestGraph::new()),
            next_node_id: AtomicU64::new(0),
            monotonic_watermark_ns: AtomicU64::new(0),
            metrics,
        }
    }

    pub(crate) fn lock_graph(&self) -> Result<RwLockWriteGuard<'_, ManifestGraph>, PluginError> {
        // TODO (L3) [lifecycle_checkpoint]: acquire the write lock on the manifest graph, returning Halt on poisoning.; slop: write lock never acquired, concurrent checkpoint writes corrupt the manifest graph.
        Err(PluginError::Halt("checkpoint graph lock poisoned".into()))
    }

    pub(crate) fn read_graph(&self) -> Result<RwLockReadGuard<'_, ManifestGraph>, PluginError> {
        // TODO (L3) [lifecycle_checkpoint]: acquire the read lock on the manifest graph, returning Halt on poisoning.; slop: read lock never acquired, readers see a half-written manifest graph.
        Err(PluginError::Halt("checkpoint graph lock poisoned".into()))
    }
}

pub(crate) fn build_checkpoint_ops(metrics: Arc<MetricRegistry>) -> Arc<dyn CheckpointOps> {
    Arc::new(RealCheckpointOps::new(metrics))
}

impl CheckpointOps for RealCheckpointOps {
    // see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
    fn snapshot(
        &self,
        _lineage: PidLineage,
        _base: SnapshotDescriptor,
    ) -> Result<u64, PluginError> {
        // TODO (L3) [lifecycle_checkpoint]: insert a checkpoint node for the lineage head and return its node id.; slop: node never inserted, the lineage head points at a missing node.
        Err(PluginError::Halt("checkpoint snapshot pending (L3)".into()))
    }

    // see: cans/08-lifecycle.md#CoW-fork
    fn fork(&self, _node_id: u64, _lineage: PidLineage) -> Result<(), PluginError> {
        // TODO (L3) [lifecycle_checkpoint]: branch a CoW fork chain from the node for the lineage.; slop: fork chain never branched, CoW writes land on the shared parent.
        Err(PluginError::Halt("checkpoint fork pending (L3)".into()))
    }

    // see: cans/08-lifecycle.md#Delta-chain-compaction
    fn branch_from(&self, _node_id: u64, _lineage: PidLineage) -> Result<(), PluginError> {
        // TODO (L3) [lifecycle_checkpoint]: set the lineage head to the node after validating it exists and the lineage is unheaded.; slop: head set without validation, the lineage points at a nonexistent node.
        Err(PluginError::Halt("checkpoint branch pending (L3)".into()))
    }

    // see: cans/09-snapshots.md#Restore-modes
    // see: cans/08-lifecycle.md#Clock-invariants
    fn rollback(
        &self,
        _node_id: u64,
        _guest_monotonic_ns: u64,
    ) -> Result<SnapshotDescriptor, PluginError> {
        // TODO (L3) [lifecycle_checkpoint]: restore the node descriptor, clamping the guest clock to the monotonic watermark.; slop: clock never clamped, restored guest time jumps backwards past the watermark.
        Err(PluginError::Halt("checkpoint rollback pending (L3)".into()))
    }

    fn head(&self, _lineage: PidLineage) -> Option<u64> {
        // TODO (L3) [lifecycle_checkpoint]: return the head node id for the lineage, if any.; slop: head lookup always returns None, restore finds no checkpoint.
        None
    }

    fn lineage(&self, _node_id: u64) -> Result<Vec<u64>, PluginError> {
        // TODO (L3) [lifecycle_checkpoint]: return the ancestor chain for the node.; slop: ancestor chain always empty, rollback cannot find its parent.
        Err(PluginError::Halt("checkpoint lineage pending (L3)".into()))
    }

    // see: cans/08-lifecycle.md#Clock-invariants
    fn monotonic_watermark_ns(&self) -> u64 {
        // TODO (L3) [lifecycle_checkpoint]: return the monotonic watermark in nanoseconds.; slop: watermark always reads zero, the clock clamp has nothing to clamp against.
        0
    }

    // see: cans/08-lifecycle.md#Delta-chain-compaction
    // see: cans/09-snapshots.md#Groups-and-lineage
    fn compact(&self, _lineage: PidLineage) -> Result<u64, PluginError> {
        // TODO (L3) [lifecycle_checkpoint]: compact the lineage delta chain into a single node and return its node id.; slop: delta chain grows unbounded, restore replays an ever-longer chain.
        Err(PluginError::Halt(
            "checkpoint compaction pending (L3)".into(),
        ))
    }

    fn fresh(&self) -> Arc<dyn CheckpointOps> {
        Arc::new(Self::new(Arc::clone(&self.metrics)))
    }
}
