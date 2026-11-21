// see: cans/plugins/ledger.md#Plugin-ledger
#![forbid(unsafe_code)]

// TODO (L3) [telemetry_audit]: tests/test_merkle_wal.rs, test_causal_delta.rs, test_otel.rs per cans/artifacts/config/project-structure.txt; slop: ledger tests missing, WAL/OTel paths untested, encoder-only passes vacuously
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use kelir_interface::plugin_model::PluginDeps;
use kelir_interface::telemetry::CausalDelta;
use kelir_interface::telemetry::MetricRegistry;

pub mod compute_ledger;
#[path = "ledger_plugin.flow.rs"]
pub mod ledger_plugin;
#[path = "ledger_plugin.run.rs"]
pub mod ledger_plugin_run;
#[path = "otel_ledger.flow.rs"]
pub mod otel_ledger;
#[path = "otlp_encoder.flow.rs"]
pub mod otlp_encoder;
#[path = "otlp_encoder.run.rs"]
pub mod otlp_encoder_run;
#[path = "otlp_exporter.flow.rs"]
pub mod otlp_exporter;
#[path = "otlp_exporter.run.rs"]
pub mod otlp_exporter_run;
#[path = "otlp_records.flow.rs"]
pub mod otlp_records;
#[path = "otlp_span.flow.rs"]
pub mod otlp_span;
#[path = "wal.run.rs"]
pub mod wal;
#[path = "wal_ledger.flow.rs"]
pub mod wal_ledger;

pub use otel_ledger::TraceParent;
pub use otlp_encoder::OtlpEncoder;
pub use otlp_exporter::{OtlpEndpoint, OtlpError, OtlpExporter, OtlpExporterConfig};
pub use otlp_records::{OtlpCausalDeltaRecord, OtlpPhaseTimingRecord, OtlpReceiptRecord};
pub use otlp_span::OtlpSpan;
pub use wal_ledger::{
    AuditDropCounters, DualHash, LedgerSink, LocalMerkleDbSink, MerkleChain, MerkleInclusionProof,
    OcsfClassId, OtlpLedgerSink, SignedTreeHead, WalRecord, WalSegmentHeader, WitnessClient,
    WitnessCosignature, dual_merkle_leaf, dual_merkle_node, dual_merkle_root, inclusion_proof,
    pack_segment_header, verify_segment_frame,
};

// see: cans/plugins/ledger.md#OpenTelemetry
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OtelConfig {
    pub endpoint: String,
    pub service_name: String,
    pub propagate_w3c_traceparent: bool,
    pub export_diagnostic_pmu_gauges: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReceiptConfig {
    pub include_dirty_bytes: bool,
    pub include_binary_blake3: bool,
    pub include_syscall_count: bool,
    pub include_wall_clock_ns: bool,
    pub include_phase_timings: bool,
}

// see: cans/plugins/ledger.md#Invariants-and-policy-(ledger)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LedgerConfig {
    // see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.store_path
    pub store_path: PathBuf,
    pub merkle_chain: bool,
    // see: cans/plugins/ledger.md#Strict-audit-and-drop-accounting
    pub strict: bool,
    // see: cans/plugins/ledger.md#Strict-audit-and-drop-accounting
    pub ocsf_mapping: bool,
    pub suppress_local_db: bool,
    pub hashing_standard: HashingStandard,
    pub record_pre_redaction_hash: bool,
    pub include_hook_spans_in_root: bool,
    pub witness_cosigning_endpoint: Option<String>,
    pub otel: Option<OtelConfig>,
    pub receipts: ReceiptConfig,
}

// see: cans/artifacts/examples/autonomous-swe-agent.rs#FinalizeRequest
#[derive(Debug, Clone)]
pub struct FinalizeRequest {
    pub include_delta: Option<CausalDelta>,
    pub include_merkle_root: bool,
}

// see: cans/artifacts/examples/autonomous-swe-agent.rs#FinalReceipt
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FinalReceipt {
    pub merkle_root: String,
    pub signed_jws: String,
    pub causal_delta: Option<CausalDelta>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.hashing_standard
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HashingStandard {
    #[default]
    Rfc6962DualBlake3Sha256,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MerkleWalConfig {
    // see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.store_path
    pub store_path: PathBuf,
    pub hashing_standard: HashingStandard,
    pub sign_leaves: bool,
    pub fsync_on_reap: bool,
    pub record_pre_redaction_hash: bool,
    pub include_hook_spans_in_root: bool,
    pub witness_cosigning_endpoint: Option<String>,
    // see: cans/plugins/ledger.md#Merkle-WAL-specification
    pub kms_provider: Option<String>,
    // see: cans/plugins/ledger.md#Merkle-WAL-specification
    pub min_signing_key_bytes: u32,
    // see: cans/plugins/ledger.md#Merkle-WAL-specification
    pub verify_key_fingerprint: bool,
}

// see: cans/plugins/ledger.md#Invariants-and-policy-(ledger)
#[derive(Clone, Default)]
pub struct LedgerCtx {
    denials: Arc<Mutex<HashMap<u32, u64>>>,
    otel: OtelConfig,
    metrics: Option<Arc<MetricRegistry>>,
    merkle: MerkleWalConfig,
    chain: Arc<MerkleChain>,
}

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
// see: cans/plugins/ledger.md#Invariants-and-policy-(ledger)
pub type LedgerHandle = LedgerCtx;

#[derive(Clone, PluginDeps)]
pub struct LedgerPlugin {
    pub merkle_enabled: bool,
    pub otel: OtelConfig,
    pub config: LedgerConfig,
    merkle_wal: MerkleWalConfig,
    chain: Arc<MerkleChain>,
    denials: Arc<Mutex<HashMap<u32, u64>>>,
    metrics: Option<Arc<MetricRegistry>>,
}

// see: cans/02-plugin-model.md#KelirPlugin-trait
