#![forbid(unsafe_code)]

pub mod registry;

use kelir_interface::telemetry::{ExecutionReceipt, PhaseTimings};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OtlpError {
    #[error("OTLP encode error: {0}")]
    Encode(String),
    #[error("OTLP transport error: {0}")]
    Transport(String),
}

pub struct OtlpExporter {
    pub endpoint: String,
}

impl OtlpExporter {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
        }
    }

    pub fn export_receipt(&self, receipt: &ExecutionReceipt) -> Result<Vec<u8>, OtlpError> {
        let _ = receipt;
        // see: cans/10-attestation.md#Execution-receipts
        Err(OtlpError::Encode("receipt export (L3)".to_string()))
    }

    pub fn export_phase_timings(&self, timings: &PhaseTimings) -> Result<Vec<u8>, OtlpError> {
        let _ = timings;
        // see: cans/10-attestation.md#Execution-receipts
        Err(OtlpError::Encode("phase timings export (L3)".to_string()))
    }
}
