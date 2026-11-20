// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
use serde::{Deserialize, Serialize};

use super::ops_flow::CheckpointOpsArc;
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
    fn fresh(&self) -> CheckpointOpsArc;
}

// see: cans/09-snapshots.md#Groups-and-lineage
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StepDagConfig {
    pub enabled: bool,
    pub require_signed_attribution: bool,
    pub require_separation_of_duties: bool,
    pub storage: String,
}

// see: cans/08-lifecycle.md#Delta-chain-compaction
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompactionConfig {
    pub unbounded_depth: bool,
    pub lazy_cow_sharing: bool,
    pub compaction_on_export: bool,
}

// see: cans/08-lifecycle.md#Zero-harness-boundary-detection
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BoundaryConfig {
    pub sentinel_path: String,
    pub detect_pty_quiescence: bool,
    pub pre_checkpoint_dentry_trim: bool,
}

// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CheckpointPlaneConfig {
    pub step_dag: StepDagConfig,
    pub compaction: CompactionConfig,
    pub snapshot_boundaries: BoundaryConfig,
}
