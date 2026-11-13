#![forbid(unsafe_code)]

#[path = "encoder.flow.rs"]
pub mod encoder;
#[path = "encoder.run.rs"]
pub mod encoder_run;
#[path = "exporter.flow.rs"]
pub mod exporter;
#[path = "exporter.run.rs"]
pub mod exporter_run;
#[path = "records_otlp.flow.rs"]
pub mod records_otlp;
#[path = "registry_otlp.flow.rs"]
pub mod registry_otlp;
#[path = "span.flow.rs"]
pub mod span;

pub use encoder::OtlpEncoder;
pub use exporter::{OtlpEndpoint, OtlpExporter, OtlpExporterConfig};
pub use records_otlp::{OtlpCausalDeltaRecord, OtlpPhaseTimingRecord, OtlpReceiptRecord};
pub use registry_otlp::{MetricRegistry, MetricSnapshot};
pub use span::OtlpSpan;
pub(crate) use span::phase_spans;

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
