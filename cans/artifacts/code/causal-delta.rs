// crates/kelir-interface/src/telemetry/delta.rs
//
// Differential state delta between two checkpoints: Δ = S_A - S_B.
// Δ = S_A - S_B over memory, inodes, and wire egress.
// Computes exactly what an agent branch did differently from its parent.
//
// Canonical spec home: cans/08-epistemic.md#Causal-delta

use std::path::PathBuf;

pub struct CausalDelta {
    /// Hardware-agnostic byte-level RAM delta
    pub memory_diff_bytes: u64,
    /// Modified inodes between the two checkpoints
    pub modified_inodes: Vec<PathBuf>,
    pub spawned_pids: Vec<u32>,
    /// Wire egress bytes between the two checkpoints
    pub egress_bytes: u64,
}
