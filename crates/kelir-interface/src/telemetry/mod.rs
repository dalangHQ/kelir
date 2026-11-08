// see: cans/10-attestation.md#Execution-receipts
// see: cans/10-attestation.md#Causal-delta

pub mod delta;
pub mod merkle;
pub mod receipt;

pub use delta::CausalDelta;
pub use receipt::{AttestationToken, ExecutionReceipt, ExitKind, PhaseTimings, TelemetryExtension};
