// see: cans/artifacts/code/causal-delta.rs#CausalDelta
// see: cans/10-attestation.md#Causal-delta
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CausalDelta {
    pub memory_diff_bytes: u64,
    pub modified_inodes: Vec<PathBuf>,
    pub spawned_pids: Vec<u32>,
    pub egress_bytes: u64,
    #[serde(with = "crate::hex_codec::serde_hex_32")]
    pub storage_delta_root: [u8; 32],
    #[serde(with = "crate::hex_codec::serde_hex_32")]
    pub receipt_leaf_hash: [u8; 32],
}

impl CausalDelta {
    // see: cans/artifacts/code/causal-delta.rs#CausalDelta
    pub fn new(
        memory_diff_bytes: u64,
        modified_inodes: Vec<PathBuf>,
        spawned_pids: Vec<u32>,
        egress_bytes: u64,
        storage_delta_root: [u8; 32],
        receipt_leaf_hash: [u8; 32],
    ) -> Self {
        Self {
            memory_diff_bytes,
            modified_inodes,
            spawned_pids,
            egress_bytes,
            storage_delta_root,
            receipt_leaf_hash,
        }
    }
}
