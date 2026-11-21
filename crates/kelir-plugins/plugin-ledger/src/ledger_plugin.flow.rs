// see: cans/plugins/ledger.md#Plugin-ledger
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use kelir_interface::plugin_model::{KelirPlugin, PluginError};
use kelir_interface::telemetry::{CausalDelta, MetricRegistry, MetricSnapshot};

use crate::otlp_exporter::{OtlpEndpoint, OtlpError, OtlpExporter, OtlpExporterConfig};
use crate::wal_ledger::{
    AuditDropCounters, DualHash, LedgerSink, LocalMerkleDbSink, MerkleChain, OtlpLedgerSink,
    WitnessClient,
};
use crate::{
    FinalReceipt, FinalizeRequest, HashingStandard, LedgerConfig, LedgerCtx, LedgerPlugin,
    MerkleWalConfig, OtelConfig, ReceiptConfig,
};

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

impl LedgerConfig {
    pub fn new(store_path: impl Into<PathBuf>, merkle_chain: bool) -> Self {
        Self {
            store_path: store_path.into(),
            merkle_chain,
            suppress_local_db: false,
            hashing_standard: HashingStandard::Rfc6962DualBlake3Sha256,
            record_pre_redaction_hash: false,
            include_hook_spans_in_root: false,
            witness_cosigning_endpoint: None,
            otel: None,
            receipts: ReceiptConfig::default(),
            strict: false,
            ocsf_mapping: false,
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

impl HashingStandard {
    // see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.hashing_standard
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "RFC_6962_DUAL_BLAKE3_SHA256" => Some(Self::Rfc6962DualBlake3Sha256),
            _ => None,
        }
    }
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.hashing_standard
impl From<&str> for HashingStandard {
    fn from(name: &str) -> Self {
        Self::from_name(name).unwrap_or_default()
    }
}

impl Default for MerkleWalConfig {
    fn default() -> Self {
        Self {
            store_path: PathBuf::from(".kelir/audit.merkle.db"),
            hashing_standard: HashingStandard::Rfc6962DualBlake3Sha256,
            sign_leaves: true,
            fsync_on_reap: true,
            record_pre_redaction_hash: true,
            include_hook_spans_in_root: true,
            witness_cosigning_endpoint: None,
            kms_provider: None,
            min_signing_key_bytes: 32,
            verify_key_fingerprint: false,
        }
    }
}

impl MerkleWalConfig {
    // see: cans/plugins/ledger.md#Merkle-WAL-specification
    pub fn validate(&self) -> Result<(), PluginError> {
        if self.kms_provider.is_some() && self.min_signing_key_bytes < 32 {
            return Err(PluginError::Halt(format!(
                "merkle_wal.min_signing_key_bytes {} is below the 32-byte KMS floor",
                self.min_signing_key_bytes
            )));
        }
        Ok(())
    }
}

impl LedgerCtx {
    pub fn new(otel: OtelConfig, metrics: Option<Arc<MetricRegistry>>) -> Self {
        Self {
            denials: Arc::new(Mutex::new(HashMap::new())),
            otel,
            metrics,
            merkle: MerkleWalConfig::default(),
            chain: Arc::new(MerkleChain::new()),
            strict: false,
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

    // see: cans/plugins/ledger.md#Strict-audit-and-drop-accounting
    pub fn check_backpressure(&self, stalled: bool) -> Result<(), PluginError> {
        if self.strict && stalled {
            return Err(PluginError::Halt(
                "ledger.strict backpressure halt: audit sink stalled".into(),
            ));
        }
        Ok(())
    }

    // see: cans/plugins/ledger.md#Strict-audit-and-drop-accounting
    pub fn fail_closed_on_drops(&self, counters: &AuditDropCounters) -> Result<(), PluginError> {
        if self.strict && counters.total() > 0 {
            return Err(PluginError::Halt(format!(
                "ledger.strict backpressure halt: {} audit records dropped",
                counters.total()
            )));
        }
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

    pub async fn finalize(&self, request: FinalizeRequest) -> Result<FinalReceipt, PluginError> {
        // see: cans/plugins/ledger.md#Finalize-pin-(SSOT-A1)
        let mut leaves: Vec<[u8; 32]> = Vec::new();
        if let Some(delta) = &request.include_delta {
            leaves.push(delta.receipt_leaf_hash);
        }
        let root = if request.include_merkle_root {
            let chain_root = self.chain.root(self.merkle.include_hook_spans_in_root)?;
            leaves.push(chain_root);
            crate::wal_ledger::merkle_root(&leaves)
        } else {
            crate::wal_ledger::merkle_root(&leaves)
        };
        // TODO (L3) [telemetry_audit]: sign the receipt JWS with the host session Ed25519 key; slop: receipt ships with an empty JWS, unverifiable.
        Ok(FinalReceipt {
            merkle_root: kelir_interface::hex_codec::encode_hex(&root),
            signed_jws: String::new(),
            causal_delta: request.include_delta.clone(),
        })
    }

    // see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.record_pre_redaction_hash
    pub fn record_pre_redaction_hash(&self, record_bytes: &[u8]) -> Result<DualHash, PluginError> {
        if !self.merkle.record_pre_redaction_hash {
            return Ok(crate::wal_ledger::dual_merkle_leaf(record_bytes));
        }
        self.chain.record_pre_redaction_hash(record_bytes)
    }

    // see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.include_hook_spans_in_root
    pub fn record_hook_span(
        &self,
        span: kelir_interface::telemetry::HookSpan,
    ) -> Result<(), PluginError> {
        self.chain.record_hook_span(span)
    }

    // see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.witness_cosigning_endpoint
    pub fn witness_client(&self) -> Option<WitnessClient> {
        self.merkle
            .witness_cosigning_endpoint
            .clone()
            .map(WitnessClient::new)
    }

    pub async fn get_causal_delta_from_parent(&self) -> Result<CausalDelta, PluginError> {
        // TODO (L3) [ar_09_causal_delta_aggregation_merkle]: assemble the delta from the fork StorageDelta plus the parent ExecutionReceipt via compute_ledger::compute_causal_delta.; slop: delta fabricated, merkle aggregation skipped
        Err(PluginError::Halt(
            "ledger get_causal_delta_from_parent needs the fork storage delta".into(),
        ))
    }
}

impl LedgerPlugin {
    pub fn new(merkle_enabled: bool) -> Self {
        Self {
            merkle_enabled,
            otel: OtelConfig::default(),
            config: LedgerConfig::new(".kelir/audit.merkle.db", merkle_enabled),
            merkle_wal: MerkleWalConfig::default(),
            chain: Arc::new(MerkleChain::new()),
            denials: Arc::new(Mutex::new(HashMap::new())),
            metrics: None,
        }
    }

    pub fn with_merkle_wal(mut self, merkle_wal: MerkleWalConfig) -> Self {
        self.merkle_wal = merkle_wal;
        self
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
            Arc::new(LocalMerkleDbSink::new(self.config.store_path.clone()))
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
            merkle: self.merkle_wal.clone(),
            chain: Arc::clone(&self.chain),
            strict: self.config.strict,
        }
    }
    fn name(&self) -> &'static str {
        "ledger"
    }
}
