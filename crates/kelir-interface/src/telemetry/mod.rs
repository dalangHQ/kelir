// see: cans/10-attestation.md#Execution-receipts
// see: cans/10-attestation.md#Causal-delta

#[path = "delta_attest.flow.rs"]
pub mod delta_attest;
#[path = "merkle_attest.flow.rs"]
pub mod merkle_attest;
#[path = "receipt_attest.flow.rs"]
pub mod receipt_attest;

pub use delta_attest::CausalDelta;
pub use receipt_attest::{
    AttestationToken, ExecutionReceipt, ExitKind, PhaseTimings, ReceiptExtension, ReceiptStore,
};
