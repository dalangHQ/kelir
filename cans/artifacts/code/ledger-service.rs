// crates/kelir-plugins/plugin-ledger/src/lib.rs
//
// The ledger finalize surface: what the ledger plugin exposes for closing
// the Merkle WAL into a signed receipt. FinalizeRequest and FinalReceipt
// are defined here, not in the examples: the live see: refs that pointed
// at cans/artifacts/examples/autonomous-swe-agent.rs#FinalizeRequest and
// #FinalReceipt named anchors that hold only TODO comments, not the
// definitions. All three SDKs agree on the shape: finalize takes the
// causal delta plus a merkle-root flag and returns the finalized receipt.
//
// Owned by: cans/plugins/ledger.md#Plugin-ledger

use kelir_interface::plugin_model::PluginError;
use kelir_interface::telemetry::CausalDelta;

/// Request to finalize the Merkle WAL. include_delta is the causal delta
/// from the parent (Option<CausalDelta>); None finalizes without a delta.
/// All three SDKs agree on the shape: TS/Py take an optional delta and
/// both Rust acceptance examples pass Some(delta).
#[derive(Debug, Clone)]
pub struct FinalizeRequest {
    pub include_delta: Option<CausalDelta>,
    pub include_merkle_root: bool,
}

/// Finalized ledger receipt: the Merkle root plus its signed JWS, plus
/// the causal delta the receipt was finalized with when one was given.
/// The delta appearing here is the WS-E acceptance: a finalized receipt
/// carries the delta it was closed over.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FinalReceipt {
    pub merkle_root: String,
    pub signed_jws: String,
    pub causal_delta: Option<CausalDelta>,
}

/// Ledger context handle. finalize appends the leaf via the Merkle WAL,
/// signs the receipt, and returns the finalized root.
pub struct LedgerCtx;

impl LedgerCtx {
    /// Finalizes the ledger receipt into a FinalReceipt.
    /// Returns Result<FinalReceipt, PluginError>: the finalized root on
    /// success, never the receipt itself unwrapped.
    pub async fn finalize(&self, _request: FinalizeRequest) -> Result<FinalReceipt, PluginError> {
        // TODO (L3): append the leaf via wal_ledger::WalRecord + wal_ledger::merkle_root, sign the receipt, and return the finalized root.
        Err(PluginError::Halt(
            "ledger finalize needs the Merkle WAL sink".into(),
        ))
    }
}
