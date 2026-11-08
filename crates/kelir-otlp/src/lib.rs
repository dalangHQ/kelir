#![forbid(unsafe_code)]

pub mod registry;

pub use registry::{MetricRegistry, MetricSnapshot};

use kelir_interface::telemetry::{CausalDelta, ExecutionReceipt, PhaseTimings};
use thiserror::Error;

// see: cans/14-observability.md#Metrics-pipeline
#[derive(Error, Debug)]
pub enum OtlpError {
    #[error("OTLP encode error: {0}")]
    Encode(String),
    #[error("OTLP transport error: {0}")]
    Transport(String),
    #[error("OTLP export not implemented (L3): {0}")]
    NotImplemented(String),
    #[error("OTLP endpoint invalid: {0}")]
    InvalidEndpoint(String),
}

// see: cans/14-observability.md#Metrics-pipeline
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtlpEndpoint {
    pub url: String,
}

impl OtlpEndpoint {
    pub fn new(url: impl Into<String>) -> Result<Self, OtlpError> {
        let url = url.into();
        if url.starts_with("http://") || url.starts_with("https://") {
            Ok(Self { url })
        } else {
            Err(OtlpError::InvalidEndpoint(url))
        }
    }
}

// see: cans/14-observability.md#Metrics-pipeline
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtlpExporterConfig {
    pub endpoint: OtlpEndpoint,
    pub service_name: String,
}

impl OtlpExporterConfig {
    pub fn new(endpoint: OtlpEndpoint, service_name: impl Into<String>) -> Self {
        Self {
            endpoint,
            service_name: service_name.into(),
        }
    }
}

// see: cans/14-observability.md#Metrics-pipeline
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtlpSpan {
    pub name: String,
    pub start_ns: u64,
    pub end_ns: u64,
}

impl OtlpSpan {
    pub fn new(name: impl Into<String>, start_ns: u64, end_ns: u64) -> Self {
        Self {
            name: name.into(),
            start_ns,
            end_ns,
        }
    }
}

// see: cans/14-observability.md#Metrics-pipeline
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtlpReceiptRecord {
    pub pid: u32,
    pub binary_path: String,
    pub binary_blake3: String,
    pub exit_code: i32,
    pub wall_clock_ns: u64,
    pub spans: Vec<OtlpSpan>,
}

impl OtlpReceiptRecord {
    pub fn from_receipt(receipt: &ExecutionReceipt) -> Self {
        let mut spans = Vec::new();
        if let Some(timings) = &receipt.phase_timings {
            let mut start_ns = 0;
            for (name, duration_ns) in [
                ("trap_ingress", timings.trap_ingress_ns),
                ("policy", timings.policy_ns),
                ("ffi_hop", timings.ffi_hop_ns),
                ("verdict_egress", timings.verdict_egress_ns),
            ] {
                spans.push(OtlpSpan::new(name, start_ns, start_ns + duration_ns));
                start_ns += duration_ns;
            }
        }
        Self {
            pid: receipt.pid,
            binary_path: receipt.binary_path.clone(),
            binary_blake3: receipt.binary_blake3.clone(),
            exit_code: receipt.exit_code,
            wall_clock_ns: receipt.wall_clock_ns,
            spans,
        }
    }
}

// see: cans/14-observability.md#Metrics-pipeline
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OtlpPhaseTimingRecord {
    pub trap_ingress_ns: u64,
    pub ffi_hop_ns: u64,
    pub policy_ns: u64,
    pub verdict_egress_ns: u64,
}

impl OtlpPhaseTimingRecord {
    pub fn from_timings(timings: &PhaseTimings) -> Self {
        Self {
            trap_ingress_ns: timings.trap_ingress_ns,
            ffi_hop_ns: timings.ffi_hop_ns,
            policy_ns: timings.policy_ns,
            verdict_egress_ns: timings.verdict_egress_ns,
        }
    }
}

// see: cans/14-observability.md#Metrics-pipeline
fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[usize::from(*byte >> 4)] as char);
        out.push(HEX[usize::from(*byte & 0x0f)] as char);
    }
    out
}

// see: cans/14-observability.md#Metrics-pipeline
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtlpCausalDeltaRecord {
    pub memory_diff_bytes: u64,
    pub modified_inodes: Vec<String>,
    pub spawned_pids: Vec<u32>,
    pub egress_bytes: u64,
    pub storage_delta_root: String,
    pub receipt_leaf_hash: String,
}

impl OtlpCausalDeltaRecord {
    pub fn from_delta(delta: &CausalDelta) -> Self {
        Self {
            memory_diff_bytes: delta.memory_diff_bytes,
            modified_inodes: delta
                .modified_inodes
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
            spawned_pids: delta.spawned_pids.clone(),
            egress_bytes: delta.egress_bytes,
            storage_delta_root: hex_lower(&delta.storage_delta_root),
            receipt_leaf_hash: hex_lower(&delta.receipt_leaf_hash),
        }
    }
}

// see: cans/14-observability.md#Metrics-pipeline
#[derive(Debug, Clone, Default)]
pub struct OtlpEncoder;

impl OtlpEncoder {
    pub fn new() -> Self {
        Self
    }

    pub fn encode_receipt(&self, record: &OtlpReceiptRecord) -> Result<Vec<u8>, OtlpError> {
        let _ = record;
        // TODO (L3): encode the receipt record into an OTLP protobuf payload.
        Err(OtlpError::NotImplemented(
            "encode_receipt OTLP protobuf encoding (L3)".to_string(),
        ))
    }

    pub fn encode_phase_timings(
        &self,
        record: &OtlpPhaseTimingRecord,
    ) -> Result<Vec<u8>, OtlpError> {
        let _ = record;
        // TODO (L3): encode the phase-timing record into an OTLP protobuf payload.
        Err(OtlpError::NotImplemented(
            "encode_phase_timings OTLP protobuf encoding (L3)".to_string(),
        ))
    }

    pub fn encode_span(&self, span: &OtlpSpan) -> Result<Vec<u8>, OtlpError> {
        let _ = span;
        // TODO (L3): encode the span into an OTLP protobuf payload.
        Err(OtlpError::NotImplemented(
            "encode_span OTLP protobuf encoding (L3)".to_string(),
        ))
    }

    pub fn encode_causal_delta(
        &self,
        record: &OtlpCausalDeltaRecord,
    ) -> Result<Vec<u8>, OtlpError> {
        let _ = record;
        // TODO (L3): encode the causal-delta record into an OTLP protobuf payload.
        Err(OtlpError::NotImplemented(
            "encode_causal_delta OTLP protobuf encoding (L3)".to_string(),
        ))
    }

    pub fn encode_metrics(&self, snapshot: &MetricSnapshot) -> Result<Vec<u8>, OtlpError> {
        let _ = snapshot;
        // TODO (L3): encode the metric snapshot into an OTLP protobuf payload.
        Err(OtlpError::NotImplemented(
            "encode_metrics OTLP protobuf encoding (L3)".to_string(),
        ))
    }
}

pub struct OtlpExporter {
    pub config: OtlpExporterConfig,
    pub encoder: OtlpEncoder,
}

impl OtlpExporter {
    pub fn new(endpoint: impl Into<String>) -> Result<Self, OtlpError> {
        Ok(Self {
            config: OtlpExporterConfig::new(OtlpEndpoint::new(endpoint)?, "kelir"),
            encoder: OtlpEncoder::new(),
        })
    }

    pub fn with_config(config: OtlpExporterConfig) -> Self {
        Self {
            config,
            encoder: OtlpEncoder::new(),
        }
    }

    pub fn export_receipt(&self, receipt: &ExecutionReceipt) -> Result<Vec<u8>, OtlpError> {
        let record = OtlpReceiptRecord::from_receipt(receipt);
        self.encoder.encode_receipt(&record)
    }

    pub fn export_phase_timings(&self, timings: &PhaseTimings) -> Result<Vec<u8>, OtlpError> {
        let record = OtlpPhaseTimingRecord::from_timings(timings);
        self.encoder.encode_phase_timings(&record)
    }

    pub fn export_span(&self, span: &OtlpSpan) -> Result<Vec<u8>, OtlpError> {
        self.encoder.encode_span(span)
    }

    pub fn export_causal_delta(&self, delta: &CausalDelta) -> Result<Vec<u8>, OtlpError> {
        let record = OtlpCausalDeltaRecord::from_delta(delta);
        self.encoder.encode_causal_delta(&record)
    }

    pub fn export_metrics(&self, snapshot: &MetricSnapshot) -> Result<Vec<u8>, OtlpError> {
        self.encoder.encode_metrics(snapshot)
    }
}
