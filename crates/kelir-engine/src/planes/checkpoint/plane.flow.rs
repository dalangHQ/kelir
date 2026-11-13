// see: cans/01-architecture.md#Native-Engine-Planes-on-Session
use super::*;
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

    // see: cans/10-attestation.md#Step-evaluation-loop
    pub fn create_step(
        &self,
        lineage: PidLineage,
        base: SnapshotDescriptor,
    ) -> Result<u64, PluginError> {
        self.ops.create_step(lineage, base)
    }

    // see: cans/10-attestation.md#Step-evaluation-loop
    pub fn active_step(
        &self,
        lineage: PidLineage,
    ) -> Result<Option<SnapshotDescriptor>, PluginError> {
        self.ops.active_step(lineage)
    }

    // see: cans/08-lifecycle.md#Delta-chain-compaction
    pub fn commit_step(&self, lineage: PidLineage) -> Result<u64, PluginError> {
        self.ops.commit_step(lineage)
    }

    // see: cans/08-lifecycle.md#Delta-chain-compaction
    pub fn rollback_step(&self, lineage: PidLineage) -> Result<SnapshotDescriptor, PluginError> {
        self.ops.rollback_step(lineage)
    }

    // see: cans/08-lifecycle.md#Delta-chain-compaction
    pub fn branch_from(&self, node_id: u64, lineage: PidLineage) -> Result<(), PluginError> {
        self.ops.branch_from(node_id, lineage)
    }

    // see: cans/09-snapshots.md#Restore-modes
    // see: cans/08-lifecycle.md#Clock-invariants
    pub fn restore(
        &self,
        node_id: u64,
        guest_monotonic_ns: u64,
    ) -> Result<SnapshotDescriptor, PluginError> {
        self.ops.restore(node_id, guest_monotonic_ns)
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
