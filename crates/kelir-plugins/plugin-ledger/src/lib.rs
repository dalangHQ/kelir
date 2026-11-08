// see: cans/plugins/ledger.md
#![forbid(unsafe_code)]

// TODO (L2/L3): src/wal.rs + otel.rs + compute.rs, tests/test_merkle_wal.rs, test_causal_delta.rs, test_otel.rs per cans/artifacts/config/project-structure.txt.
use std::sync::Arc;

use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{KelirPlugin, LifecycleParticipant, MountScope, TrapInterposer};
use kelir_interface::telemetry::CausalDelta;
use kelir_otlp::registry::{MetricRegistry, MetricSnapshot};
use kelir_otlp::{OtlpEndpoint, OtlpError, OtlpExporter, OtlpExporterConfig};

pub trait LedgerSink: Send + Sync {
    fn emit_delta(&self, delta: &CausalDelta) -> Result<(), PluginError>;
}

// see: cans/plugins/ledger.md#OpenTelemetry
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OtelConfig {
    pub endpoint: String,
    pub service_name: String,
    pub propagate_w3c_traceparent: bool,
    pub export_diagnostic_pmu_gauges: bool,
}

impl OtelConfig {
    pub fn new(endpoint: impl Into<String>, service_name: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            service_name: service_name.into(),
            propagate_w3c_traceparent: false,
            export_diagnostic_pmu_gauges: false,
        }
    }

    pub fn exporter_config(&self) -> Result<OtlpExporterConfig, OtlpError> {
        Ok(OtlpExporterConfig::new(
            OtlpEndpoint::new(self.endpoint.clone())?,
            self.service_name.clone(),
        ))
    }
}

#[derive(Clone)]
pub struct LedgerPlugin {
    pub merkle_enabled: bool,
    pub otel: OtelConfig,
    metrics: Option<Arc<MetricRegistry>>,
}

impl LedgerPlugin {
    pub fn new(merkle_enabled: bool) -> Self {
        Self {
            merkle_enabled,
            otel: OtelConfig::default(),
            metrics: None,
        }
    }

    pub fn with_otel(mut self, otel: OtelConfig) -> Self {
        self.otel = otel;
        self
    }

    pub fn with_metrics(mut self, metrics: Arc<MetricRegistry>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    pub fn metrics(&self) -> Option<&Arc<MetricRegistry>> {
        self.metrics.as_ref()
    }

    pub fn snapshot_metrics(&self) -> Option<MetricSnapshot> {
        self.metrics.as_ref().map(|registry| registry.snapshot())
    }

    pub fn export_metrics(&self) -> Result<Vec<u8>, OtlpError> {
        // TODO (L3): ship the encoded snapshot over OTLP gRPC/HTTP; the encoder below only shapes the payload.
        let Some(snapshot) = self.snapshot_metrics() else {
            return Err(OtlpError::NotImplemented(
                "ledger export_metrics needs a bound MetricRegistry".to_string(),
            ));
        };
        let exporter = OtlpExporter::with_config(self.otel.exporter_config()?);
        exporter.export_metrics(&snapshot)
    }
}

impl KelirPlugin for LedgerPlugin {
    type Context = ();
    fn context(&self) {}
    fn name(&self) -> &'static str {
        "ledger"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
}

impl LifecycleParticipant for LedgerPlugin {}
impl TrapInterposer for LedgerPlugin {}
