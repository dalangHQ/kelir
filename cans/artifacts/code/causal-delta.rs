// crates/kelir-interface/src/telemetry/delta.rs
//
// Differential state delta between two checkpoints: Δ = S_A - S_B.
// Δ = S_A - S_B over memory, inodes, and wire egress.
// Computes exactly what an agent branch did differently from its parent.
//
// Canonical spec home: cans/08-epistemic.md#Causal-delta

use std::path::PathBuf;

// Delta chain (07): one immutable Base and N stacked WorkingDeltas,
// depth unbounded. StepVerdict::Commit folds the current delta into the
// active checkpoint; StepVerdict::Rollback discards only the current
// delta (≤8ms, sparse upperdir delta dropped). Lineage (parent_digest
// -> child_digest) lives in the SQLite catalog, not in this struct.
pub struct CausalDelta {
    /// Hardware-agnostic byte-level RAM delta
    pub memory_diff_bytes: u64,
    /// Modified inodes between the two checkpoints
    pub modified_inodes: Vec<PathBuf>,
    pub spawned_pids: Vec<u32>,
    /// Wire egress bytes between the two checkpoints
    pub egress_bytes: u64,
    /// BLAKE3 root of modified storage inodes (from StorageDelta)
    pub storage_delta_root: [u8; 32],
    /// Merkle leaf hash of the associated ExecutionReceipt
    pub receipt_leaf_hash: [u8; 32],
}
