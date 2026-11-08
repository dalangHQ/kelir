// see: cans/artifacts/code/causal-delta.rs#CausalDelta
use std::path::PathBuf;

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CausalDelta {
    pub memory_diff_bytes: u64,
    pub modified_inodes: Vec<PathBuf>,
    pub spawned_pids: Vec<u32>,
    pub egress_bytes: u64,
    pub storage_delta_root: [u8; 32],
    pub receipt_leaf_hash: [u8; 32],
}
