// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
use std::sync::Arc;

use super::ops::CheckpointOps;

pub type CheckpointOpsArc = Arc<dyn CheckpointOps>;
