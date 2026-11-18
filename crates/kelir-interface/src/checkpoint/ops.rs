// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
use std::sync::Arc;

use crate::checkpoint::graph::PidLineage;
use crate::driver_interface::SnapshotDescriptor;
use crate::plugin_model::PluginError;

// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
pub trait CheckpointOps: Send + Sync {
    fn snapshot(&self, lineage: PidLineage, base: SnapshotDescriptor) -> Result<u64, PluginError>;
    fn fork(&self, node_id: u64, lineage: PidLineage) -> Result<(), PluginError>;
    fn branch_from(&self, node_id: u64, lineage: PidLineage) -> Result<(), PluginError>;
    fn rollback(
        &self,
        node_id: u64,
        guest_monotonic_ns: u64,
    ) -> Result<SnapshotDescriptor, PluginError>;
    fn head(&self, lineage: PidLineage) -> Option<u64>;
    fn lineage(&self, node_id: u64) -> Result<Vec<u64>, PluginError>;
    fn monotonic_watermark_ns(&self) -> u64;
    fn compact(&self, lineage: PidLineage) -> Result<u64, PluginError>;
    fn fresh(&self) -> Arc<dyn CheckpointOps>;
}
