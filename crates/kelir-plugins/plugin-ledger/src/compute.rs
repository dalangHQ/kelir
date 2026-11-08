// see: cans/10-attestation.md#Causal-delta
use std::path::PathBuf;

use kelir_interface::plugin::PluginError;
use kelir_interface::telemetry::{CausalDelta, ExecutionReceipt};

// see: cans/05-storage-plane.md#Workspace-commit
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StorageDelta {
    pub memory_diff_bytes: u64,
    pub modified_inodes: Vec<PathBuf>,
    pub spawned_pids: Vec<u32>,
    pub egress_bytes: u64,
    pub storage_delta_root: [u8; 32],
}

// see: cans/10-attestation.md#Causal-delta
pub fn compute_causal_delta(
    storage: &StorageDelta,
    receipt: &ExecutionReceipt,
) -> Result<CausalDelta, PluginError> {
    Ok(CausalDelta::new(
        storage.memory_diff_bytes,
        storage.modified_inodes.clone(),
        storage.spawned_pids.clone(),
        storage.egress_bytes,
        storage.storage_delta_root,
        receipt.leaf_hash(),
    ))
}
