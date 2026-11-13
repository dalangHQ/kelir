use kelir_interface::telemetry::{CausalDelta, ExecutionReceipt, PhaseTimings};

use crate::OtlpError;
use crate::encoder::OtlpEncoder;
use crate::records_otlp::{OtlpCausalDeltaRecord, OtlpPhaseTimingRecord, OtlpReceiptRecord};
use crate::registry_otlp::MetricSnapshot;
use crate::span::OtlpSpan;

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
