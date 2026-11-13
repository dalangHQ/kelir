// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
#![forbid(unsafe_code)]

#[path = "compaction.run.rs"]
pub mod compaction;
#[path = "manifest_graph.run.rs"]
pub mod manifest_graph;
#[path = "real_checkpoint.run.rs"]
pub mod real_checkpoint;
#[path = "step_slot.run.rs"]
pub mod step_slot;

pub use real_checkpoint::RealCheckpointOps;
