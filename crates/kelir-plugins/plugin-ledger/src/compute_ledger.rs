// see: cans/10-attestation.md#Causal-delta
use std::path::PathBuf;

pub use crate::compute_ledger_flow::compute_causal_delta;

// see: cans/05-storage-plane.md#Workspace-commit
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StorageDelta {
    pub memory_diff_bytes: u64,
    pub modified_inodes: Vec<PathBuf>,
    pub spawned_pids: Vec<u32>,
    pub egress_bytes: u64,
    pub storage_delta_root: [u8; 32],
}
