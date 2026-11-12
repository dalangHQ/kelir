// see: cans/plugins/ledger.md#Plugin-ledger
#![forbid(unsafe_code)]

// TODO (L3) [telemetry_audit]: tests/test_merkle_wal.rs, test_causal_delta.rs, test_otel.rs per cans/artifacts/config/project-structure.txt; slop: ledger tests missing, WAL/OTel paths untested, encoder-only passes vacuously
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{KelirPlugin, LifecycleParticipant, NoDeps, PluginDeps};
use kelir_interface::telemetry::CausalDelta;
use kelir_otlp::registry_otlp::{MetricRegistry, MetricSnapshot};
use kelir_otlp::{OtlpEndpoint, OtlpError, OtlpExporter, OtlpExporterConfig};

pub mod compute_ledger;
pub mod otel_ledger;
pub mod wal_ledger;

pub use otel_ledger::TraceParent;
pub use wal_ledger::{LedgerSink, LocalMerkleDbSink, OtlpLedgerSink, WalRecord};

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
    pub store: String,
    pub merkle_chain: bool,
    pub suppress_local_db: bool,
    pub otel: Option<OtelConfig>,
    pub receipts: ReceiptConfig,
}

impl LedgerConfig {
    pub fn new(store: impl Into<String>, merkle_chain: bool) -> Self {
        Self {
            store: store.into(),
            merkle_chain,
            suppress_local_db: false,
            otel: None,
            receipts: ReceiptConfig::default(),
        }
    }
}

// see: cans/plugins/ledger.md#Invariants-and-policy-(ledger)
struct NoopLedgerSink;

impl LedgerSink for NoopLedgerSink {
    fn emit_delta(&self, _delta: &CausalDelta) -> Result<(), PluginError> {
        Ok(())
    }
}

// see: cans/artifacts/examples/autonomous-swe-agent.rs#FinalizeRequest
#[derive(Debug, Clone)]
pub struct FinalizeRequest {
    pub include_delta: CausalDelta,
    pub include_merkle_root: bool,
}

// see: cans/artifacts/examples/autonomous-swe-agent.rs#FinalReceipt
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FinalReceipt {
    pub merkle_root: String,
    pub signed_jws: String,
}

// see: cans/plugins/ledger.md#Invariants-and-policy-(ledger)
#[derive(Clone, Default)]
pub struct LedgerCtx {
    denials: Arc<Mutex<HashMap<u32, u64>>>,
    otel: OtelConfig,
    metrics: Option<Arc<MetricRegistry>>,
}

impl LedgerCtx {
    pub fn new(otel: OtelConfig, metrics: Option<Arc<MetricRegistry>>) -> Self {
        Self {
            denials: Arc::new(Mutex::new(HashMap::new())),
            otel,
            metrics,
        }
    }

    pub fn record_denial(&self, ppid: u32) -> Result<(), PluginError> {
        let mut denials = self
            .denials
            .lock()
            .map_err(|_| PluginError::Halt("ledger denials lock poisoned".into()))?;
        *denials.entry(ppid).or_insert(0) += 1;
        Ok(())
    }

    pub async fn flush(&self) -> Result<(), PluginError> {
        // TODO (L3) [ta_04_otlp_grpc_metric_export_streaming]: ship the WAL flush and OTel export over gRPC/HTTP; the encoder below only shapes the payload; slop: encoder-only kept, metrics written to local disk, no gRPC frames sent
        let Some(metrics) = &self.metrics else {
            return Err(PluginError::Halt(
                "ledger flush needs a bound MetricRegistry".into(),
            ));
        };
        let snapshot = metrics.snapshot();
        // see: cans/plugins/ledger.md#Invariants-and-policy-(ledger)
        let exporter = match self.otel.exporter_config() {
            Ok(config) => OtlpExporter::with_config(config),
            Err(err) => {
                tracing::warn!("OTLP export skipped during scaffolding: {err}");
                return Ok(());
            }
        };
        if let Err(err) = exporter.export_metrics(&snapshot) {
            tracing::warn!("OTLP export skipped during scaffolding: {err}");
            return Ok(());
        }
        Ok(())
    }

    pub async fn count_denials(&self, ppid: u32) -> Result<u64, PluginError> {
        let denials = self
            .denials
            .lock()
            .map_err(|_| PluginError::Halt("ledger denials lock poisoned".into()))?;
        Ok(denials.get(&ppid).copied().unwrap_or(0))
    }

    pub async fn blake3_of_producing_process(&self, _path: &Path) -> Result<String, PluginError> {
        // TODO (L3) [telemetry_audit]: resolve the producing-process digest from the live per-process receipt; slop: digest from path string, receipt never consulted, wrong binary attributed
        Err(PluginError::Halt(
            "ledger blake3_of_producing_process needs the live receipt".into(),
        ))
    }

    pub async fn finalize(&self, _request: FinalizeRequest) -> Result<FinalReceipt, PluginError> {
        // TODO (L3) [telemetry_audit]: append the leaf via wal_ledger::WalRecord + wal_ledger::merkle_root, sign the receipt, and return the finalized root; slop: finalize keeps Err Halt, no leaf appended or signed, receipt unverifiable
        Err(PluginError::Halt(
            "ledger finalize needs the Merkle WAL sink".into(),
        ))
    }
}

#[derive(Clone)]
pub struct LedgerPlugin {
    pub merkle_enabled: bool,
    pub otel: OtelConfig,
    pub config: LedgerConfig,
    denials: Arc<Mutex<HashMap<u32, u64>>>,
    metrics: Option<Arc<MetricRegistry>>,
}

impl LedgerPlugin {
    pub fn new(merkle_enabled: bool) -> Self {
        Self {
            merkle_enabled,
            otel: OtelConfig::default(),
            config: LedgerConfig::new(".kelir/audit.merkle.db", merkle_enabled),
            denials: Arc::new(Mutex::new(HashMap::new())),
            metrics: None,
        }
    }

    pub fn with_config(mut self, config: LedgerConfig) -> Self {
        self.merkle_enabled = config.merkle_chain;
        self.otel = config.otel.clone().unwrap_or_default();
        self.config = config;
        self
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

    // see: cans/plugins/ledger.md#Invariants-and-policy-(ledger)
    pub fn sink(&self) -> Arc<dyn LedgerSink> {
        if self.config.suppress_local_db {
            return Arc::new(NoopLedgerSink);
        }
        if self.config.merkle_chain {
            Arc::new(LocalMerkleDbSink::new(self.config.store.clone()))
        } else {
            Arc::new(OtlpLedgerSink::new(self.otel.clone()))
        }
    }

    pub fn export_metrics(&self) -> Result<Vec<u8>, OtlpError> {
        // TODO (L3) [ta_04_otlp_grpc_metric_export_streaming]: ship the encoded snapshot over OTLP gRPC/HTTP; the encoder below only shapes the payload; slop: snapshot encoder-only, OTLP never sent, collector receives nothing
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
    type Context = LedgerCtx;
    fn context(&self) -> LedgerCtx {
        LedgerCtx {
            denials: Arc::clone(&self.denials),
            otel: self.otel.clone(),
            metrics: self.metrics.clone(),
        }
    }
    fn name(&self) -> &'static str {
        "ledger"
    }
}

impl LifecycleParticipant for LedgerPlugin {}

// see: cans/02-plugin-model.md#KelirPlugin-trait
impl PluginDeps for LedgerPlugin {
    type Deps = NoDeps;
}
