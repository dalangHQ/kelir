// see: cans/10-attestation.md#Execution-receipts
// see: cans/10-attestation.md#Causal-delta

#[path = "delta_attest.flow.rs"]
pub mod delta_attest;
#[path = "merkle_attest.flow.rs"]
pub mod merkle_attest;
#[path = "metric_registry.flow.rs"]
pub mod metric_registry;
#[path = "receipt_attest.flow.rs"]
pub mod receipt_attest;

pub use metric_registry::{MetricRegistry, MetricSnapshot};

pub use delta_attest::CausalDelta;
pub use receipt_attest::{
    AttestationClass, AttestationToken, ExecutionReceipt, ExitKind, FsVfsExt, HookPolicyDescriptor,
    HookSpan, MaskedRecord, PhaseTimings, ReceiptExtension, ReceiptStore, StepLineageEntry,
    VerdictKind,
};
