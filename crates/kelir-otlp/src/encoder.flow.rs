use crate::OtlpError;
use crate::records_otlp::{OtlpCausalDeltaRecord, OtlpPhaseTimingRecord, OtlpReceiptRecord};
use crate::registry_otlp::MetricSnapshot;
use crate::span::OtlpSpan;

// see: cans/14-observability.md#Metrics-pipeline
#[derive(Debug, Clone, Default)]
pub struct OtlpEncoder;

impl OtlpEncoder {
    pub fn new() -> Self {
        Self
    }

    pub fn encode_receipt(&self, record: &OtlpReceiptRecord) -> Result<Vec<u8>, OtlpError> {
        let _ = record;
        // TODO (L3) [ta_04_otlp_grpc_metric_export_streaming]: encode the receipt record into an OTLP protobuf payload.; slop: receipt bytes mistaken for export, zero gRPC frames sent
        Err(OtlpError::NotImplemented(
            "encode_receipt OTLP protobuf encoding (L3)".to_string(),
        ))
    }

    pub fn encode_phase_timings(
        &self,
        record: &OtlpPhaseTimingRecord,
    ) -> Result<Vec<u8>, OtlpError> {
        let _ = record;
        // TODO (L3) [ta_04_otlp_grpc_metric_export_streaming]: encode the phase-timing record into an OTLP protobuf payload.; slop: timing bytes mistaken for export, zero gRPC frames sent
        Err(OtlpError::NotImplemented(
            "encode_phase_timings OTLP protobuf encoding (L3)".to_string(),
        ))
    }

    pub fn encode_span(&self, span: &OtlpSpan) -> Result<Vec<u8>, OtlpError> {
        let _ = span;
        // TODO (L3) [ta_04_otlp_grpc_metric_export_streaming]: encode the span into an OTLP protobuf payload.; slop: span bytes mistaken for export, zero gRPC frames sent
        Err(OtlpError::NotImplemented(
            "encode_span OTLP protobuf encoding (L3)".to_string(),
        ))
    }

    pub fn encode_causal_delta(
        &self,
        record: &OtlpCausalDeltaRecord,
    ) -> Result<Vec<u8>, OtlpError> {
        let _ = record;
        // TODO (L3) [ta_04_otlp_grpc_metric_export_streaming]: encode the causal-delta record into an OTLP protobuf payload.; slop: delta bytes mistaken for export, zero gRPC frames sent
        Err(OtlpError::NotImplemented(
            "encode_causal_delta OTLP protobuf encoding (L3)".to_string(),
        ))
    }

    pub fn encode_metrics(&self, snapshot: &MetricSnapshot) -> Result<Vec<u8>, OtlpError> {
        let _ = snapshot;
        // TODO (L3) [ta_04_otlp_grpc_metric_export_streaming]: encode the metric snapshot into an OTLP protobuf payload.; slop: metric bytes mistaken for export, zero gRPC frames sent
        Err(OtlpError::NotImplemented(
            "encode_metrics OTLP protobuf encoding (L3)".to_string(),
        ))
    }
}
