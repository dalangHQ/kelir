// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
// see: cans/08-lifecycle.md#Delta-chain-compaction
// see: cans/09-snapshots.md#Groups-and-lineage
use std::sync::Arc;

use kelir_interface::checkpoint::{CheckpointOps, PidLineage};
use kelir_interface::driver_interface::SnapshotDescriptor;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::telemetry::MetricRegistry;

pub struct CheckpointPlane {
    ops: Arc<dyn CheckpointOps>,
    metrics: Arc<MetricRegistry>,
}

impl CheckpointPlane {
    pub fn new(ops: Arc<dyn CheckpointOps>) -> Self {
        Self {
            ops,
            metrics: Arc::new(MetricRegistry::new()),
        }
    }

    pub fn with_metrics(mut self, metrics: Arc<MetricRegistry>) -> Self {
        self.metrics = metrics;
        self
    }

    pub fn metrics(&self) -> &Arc<MetricRegistry> {
        &self.metrics
    }

    pub fn ops(&self) -> &Arc<dyn CheckpointOps> {
        &self.ops
    }

    // see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
    pub fn snapshot(
        &self,
        lineage: PidLineage,
        base: SnapshotDescriptor,
    ) -> Result<u64, PluginError> {
        self.ops.snapshot(lineage, base)
    }

    // see: cans/08-lifecycle.md#CoW-fork
    pub fn fork(&self, node_id: u64, lineage: PidLineage) -> Result<(), PluginError> {
        self.ops.fork(node_id, lineage)
    }

    // see: cans/08-lifecycle.md#Delta-chain-compaction
    pub fn branch_from(&self, node_id: u64, lineage: PidLineage) -> Result<(), PluginError> {
        self.ops.branch_from(node_id, lineage)
    }

    // see: cans/09-snapshots.md#Restore-modes
    // see: cans/08-lifecycle.md#Clock-invariants
    pub fn rollback(
        &self,
        node_id: u64,
        guest_monotonic_ns: u64,
    ) -> Result<SnapshotDescriptor, PluginError> {
        self.ops.rollback(node_id, guest_monotonic_ns)
    }

    pub fn head(&self, lineage: PidLineage) -> Option<u64> {
        self.ops.head(lineage)
    }

    pub fn lineage(&self, node_id: u64) -> Result<Vec<u64>, PluginError> {
        self.ops.lineage(node_id)
    }

    // see: cans/08-lifecycle.md#Clock-invariants
    pub fn monotonic_watermark_ns(&self) -> u64 {
        self.ops.monotonic_watermark_ns()
    }

    // see: cans/08-lifecycle.md#Delta-chain-compaction
    // see: cans/09-snapshots.md#Groups-and-lineage
    pub fn compact(&self, lineage: PidLineage) -> Result<u64, PluginError> {
        self.ops.compact(lineage)
    }
}
