// see: cans/10-attestation.md#Execution-receipts
// see: cans/10-attestation.md#Causal-delta

#[path = "delta_attest.flow.rs"]
pub mod delta_attest;
#[path = "merkle_attest.flow.rs"]
pub mod merkle_attest;
#[path = "receipt_attest.flow.rs"]
pub mod receipt_attest;
#[path = "telemetry_ring.flow.rs"]
pub mod telemetry_ring;

pub use telemetry_ring::{FeatureVector, OtlpExporterConfig, TelemetryRing, TelemetryRingConfig};

pub use delta_attest::CausalDelta;
pub use receipt_attest::{
    AttestationClass, AttestationToken, ExecutionReceipt, ExitKind, FsVfsExt, HookPolicyDescriptor,
    HookSpan, MaskedRecord, NetL7SqlExt, PhaseTimings, ReceiptExtension, ReceiptStore,
    StepLineageEntry, VerdictKind,
};
