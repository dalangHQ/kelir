// crates/kelir-interface/src/telemetry/delta.rs
//
// Counterfactual state delta between two checkpoints: Δ = S_A - S_B.
// Used by epistemic orchestration to compute exactly what an agent branch
// did differently from its parent.
//
// Canonical spec home: cans/08-epistemic.md#Causal-delta

use std::path::PathBuf;

pub struct CausalDelta {
    /// Hardware-agnostic byte-level RAM delta
    pub memory_diff_bytes: u64,
    pub modified_inodes: Vec<PathBuf>,
    pub spawned_pids: Vec<u32>,
    pub egress_bytes: u64,
}
