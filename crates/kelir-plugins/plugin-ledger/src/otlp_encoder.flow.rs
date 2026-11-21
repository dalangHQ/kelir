use crate::otlp_exporter::OtlpError;
use crate::otlp_records::{OtlpCausalDeltaRecord, OtlpPhaseTimingRecord, OtlpReceiptRecord};
use crate::otlp_span::OtlpSpan;
use kelir_interface::telemetry::MetricSnapshot;

// see: cans/14-observability.md#Metrics-pipeline
#[derive(Debug, Clone, Default)]
pub struct OtlpEncoder;

impl OtlpEncoder {
    pub fn new() -> Self {
        Self
    }

    pub fn encode_receipt(&self, record: &OtlpReceiptRecord) -> Result<Vec<u8>, OtlpError> {
        let _ = record;
        // TODO: [ta_04_otlp_grpc_metric_export_streaming]: encode the receipt record into an OTLP protobuf payload.; slop: receipt bytes mistaken for export, zero gRPC frames sent (deferred to L3)
        Err(OtlpError::NotImplemented(
 "encode_receipt OTLP protobuf encoding".to_string(),
        ))
    }

    pub fn encode_phase_timings(
        &self,
        record: &OtlpPhaseTimingRecord,
    ) -> Result<Vec<u8>, OtlpError> {
        let _ = record;
        // TODO: [ta_04_otlp_grpc_metric_export_streaming]: encode the phase-timing record into an OTLP protobuf payload.; slop: timing bytes mistaken for export, zero gRPC frames sent (deferred to L3)
        Err(OtlpError::NotImplemented(
 "encode_phase_timings OTLP protobuf encoding".to_string(),
        ))
    }

    pub fn encode_span(&self, span: &OtlpSpan) -> Result<Vec<u8>, OtlpError> {
        let _ = span;
        // TODO: [ta_04_otlp_grpc_metric_export_streaming]: encode the span into an OTLP protobuf payload.; slop: span bytes mistaken for export, zero gRPC frames sent (deferred to L3)
        Err(OtlpError::NotImplemented(
 "encode_span OTLP protobuf encoding".to_string(),
        ))
    }

    pub fn encode_causal_delta(
        &self,
        record: &OtlpCausalDeltaRecord,
    ) -> Result<Vec<u8>, OtlpError> {
        let _ = record;
        // TODO: [ta_04_otlp_grpc_metric_export_streaming]: encode the causal-delta record into an OTLP protobuf payload.; slop: delta bytes mistaken for export, zero gRPC frames sent (deferred to L3)
        Err(OtlpError::NotImplemented(
 "encode_causal_delta OTLP protobuf encoding".to_string(),
        ))
    }

    pub fn encode_metrics(&self, snapshot: &MetricSnapshot) -> Result<Vec<u8>, OtlpError> {
        let _ = snapshot;
        // TODO: [ta_04_otlp_grpc_metric_export_streaming]: encode the metric snapshot into an OTLP protobuf payload.; slop: metric bytes mistaken for export, zero gRPC frames sent (deferred to L3)
        Err(OtlpError::NotImplemented(
 "encode_metrics OTLP protobuf encoding".to_string(),
        ))
    }
}
