// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
#![forbid(unsafe_code)]

#[path = "checkpoint.flow.rs"]
pub mod checkpoint;
#[path = "compaction.run.rs"]
mod compaction;
#[path = "manifest_graph.run.rs"]
mod manifest_graph;
#[path = "real_checkpoint.run.rs"]
mod real_checkpoint;

pub use crate::checkpoint::checkpoint_ops;
