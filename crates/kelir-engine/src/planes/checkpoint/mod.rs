// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
// see: cans/08-lifecycle.md#Delta-chain-compaction
// see: cans/09-snapshots.md#Groups-and-lineage
// see: cans/10-attestation.md#Step-evaluation-loop
use std::sync::Arc;

use kelir_interface::checkpoint::{CheckpointOps, PidLineage};
use kelir_interface::driver_interface::SnapshotDescriptor;
use kelir_interface::plugin_model::PluginError;
use kelir_otlp::registry_otlp::MetricRegistry;

pub struct CheckpointPlane {
    ops: Arc<dyn CheckpointOps>,
    metrics: Arc<MetricRegistry>,
}
#[path = "plane.flow.rs"]
mod plane;
