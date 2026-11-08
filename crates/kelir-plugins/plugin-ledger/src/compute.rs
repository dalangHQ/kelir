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
    // TODO (L3): assemble the delta from storage + receipt.leaf_hash() (raw [u8; 32], extensions covered) once kelir-interface exposes a constructor for the non_exhaustive CausalDelta per cans/artifacts/code/causal-delta.rs.
    let _ = (storage, receipt);
    Err(PluginError::Halt(
        "compute_causal_delta needs a kelir-interface CausalDelta constructor (L3)".into(),
    ))
}
