// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
// see: cans/08-lifecycle.md#Delta-chain-compaction
// see: cans/09-snapshots.md#Groups-and-lineage
// see: cans/10-attestation.md#Step-evaluation-loop
#[path = "plane.flow.rs"]
mod plane;

pub use plane::CheckpointPlane;
