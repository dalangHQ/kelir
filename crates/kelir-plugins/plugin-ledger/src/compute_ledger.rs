// see: cans/10-attestation.md#Causal-delta
use std::path::PathBuf;

use kelir_interface::plugin_model::PluginError;
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
    let _ = (storage, receipt);
    Err(PluginError::Halt(
        "deferred to L3 [ar_09_causal_delta_aggregation_merkle]: causal-delta assembly pending; slop: delta fabricated, merkle aggregation skipped".into(),
    ))
}
