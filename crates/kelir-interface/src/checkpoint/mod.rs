// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
#[path = "graph.flow.rs"]
pub mod graph;
#[path = "ops.flow.rs"]
pub mod ops;

pub use graph::{CheckpointNode, ManifestGraph, PidLineage, StepSlot};
pub use ops::CheckpointOps;
