// see: cans/10-attestation.md#Causal-delta
use kelir_interface::plugin_model::PluginError;
use kelir_interface::telemetry::{CausalDelta, ExecutionReceipt};

use crate::compute_ledger::StorageDelta;

// see: cans/10-attestation.md#Causal-delta
// TODO: [ar_09_causal_delta_aggregation_merkle]: causal-delta assembly pending; slop: delta fabricated, merkle aggregation skipped (deferred to L3)
pub fn compute_causal_delta(
    storage: &StorageDelta,
    receipt: &ExecutionReceipt,
) -> Result<CausalDelta, PluginError> {
    let _ = (storage, receipt);
    Err(PluginError::Halt(
        "deferred to L3 [ar_09_causal_delta_aggregation_merkle]: causal-delta assembly pending; slop: delta fabricated, merkle aggregation skipped".into(),
    ))
}
