// see: cans/plugins/ledger.md#Plugin-ledger
#![forbid(unsafe_code)]

// TODO (L3): tests/test_merkle_wal.rs, test_causal_delta.rs, test_otel.rs per cans/artifacts/config/project-structure.txt.
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{KelirPlugin, LifecycleParticipant, MountScope, TrapInterposer};
use kelir_interface::telemetry::CausalDelta;
use kelir_otlp::registry::{MetricRegistry, MetricSnapshot};
use kelir_otlp::{OtlpEndpoint, OtlpError, OtlpExporter, OtlpExporterConfig};

pub mod compute;
pub mod otel;
pub mod wal;

pub use otel::TraceParent;
pub use wal::{LedgerSink, LocalMerkleDbSink, OtlpLedgerSink, WalRecord};

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
    pub otel: Option<OtelConfig>,
    pub receipts: ReceiptConfig,
}

impl LedgerConfig {
    pub fn new(store: impl Into<String>, merkle_chain: bool) -> Self {
        Self {
            store: store.into(),
            merkle_chain,
            otel: None,
            receipts: ReceiptConfig::default(),
        }
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

    pub fn record_denial(&self, ppid: u32) {
        let mut denials = self.denials.lock().unwrap_or_else(|e| e.into_inner());
        *denials.entry(ppid).or_insert(0) += 1;
    }

    pub async fn flush(&self) -> Result<(), PluginError> {
        // TODO (L3): ship the WAL flush and OTel export over gRPC/HTTP; the encoder below only shapes the payload.
        let Some(metrics) = &self.metrics else {
            return Err(PluginError::Halt(
                "ledger flush needs a bound MetricRegistry".into(),
            ));
        };
        let snapshot = metrics.snapshot();
        let exporter =
            OtlpExporter::with_config(self.otel.exporter_config().map_err(|e| {
                PluginError::Halt(format!("ledger flush: OTLP config invalid: {e}"))
            })?);
        exporter
            .export_metrics(&snapshot)
            .map_err(|e| PluginError::Halt(format!("ledger flush: OTLP export failed: {e}")))?;
        Ok(())
    }

    pub async fn count_denials(&self, ppid: u32) -> u64 {
        let denials = self.denials.lock().unwrap_or_else(|e| e.into_inner());
        denials.get(&ppid).copied().unwrap_or(0)
    }

    pub async fn blake3_of_producing_process(&self, _path: &Path) -> Result<String, PluginError> {
        // TODO (L3): resolve the producing-process digest from the live per-process receipt.
        Err(PluginError::Halt(
            "ledger blake3_of_producing_process needs the live receipt".into(),
        ))
    }

    pub async fn finalize(&self, _request: FinalizeRequest) -> Result<FinalReceipt, PluginError> {
        // TODO (L3): append the leaf via wal::WalRecord + wal::merkle_root, sign the receipt, and return the finalized root.
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
        if self.config.merkle_chain {
            Arc::new(LocalMerkleDbSink::new(self.config.store.clone()))
        } else {
            Arc::new(OtlpLedgerSink::new(self.otel.clone()))
        }
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
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
}

impl LifecycleParticipant for LedgerPlugin {}
impl TrapInterposer for LedgerPlugin {}
