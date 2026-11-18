// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
#[path = "attribution.rs"]
pub mod attribution;
#[path = "graph.flow.rs"]
pub mod graph;
#[path = "ops.rs"]
pub mod ops;
#[path = "step_dag.flow.rs"]
pub mod step_dag;

pub use graph::{CheckpointNode, ManifestGraph, PidLineage, StepSlot};
pub use ops::CheckpointOps;
pub use step_dag::{StepAttribution, StepDag, StepDagConfig, StepDagError, StepNode};
