// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
use std::sync::Arc;

use kelir_interface::checkpoint::CheckpointOps;
use kelir_otlp::registry_otlp::MetricRegistry;

use crate::real_checkpoint::build_checkpoint_ops;

pub fn checkpoint_ops(metrics: Arc<MetricRegistry>) -> Arc<dyn CheckpointOps> {
    build_checkpoint_ops(metrics)
}
