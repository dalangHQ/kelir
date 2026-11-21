// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use kelir_interface::plugin_model::{EventBus, KelirPlugin};

use crate::{AnomalyConfig, AnomalyCtx, AnomalyPlugin};

// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
pub const TELEMETRY_RING_DEPTH: usize = 300;
// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
pub const FEATURE_VECTOR_DIMS: usize = 16;
// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
pub const FEATURE_SYSCALL_RATE: usize = 0;
// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
pub const FEATURE_ARG_ENTROPY: usize = 1;
// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
pub const FEATURE_BURST: usize = 2;
// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
pub const FEATURE_TRAP_RATE: usize = 3;
// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
pub const FEATURE_FS_LOCK_RATE: usize = 4;
// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
pub const FEATURE_EXEC_STDOUT_BYTES: usize = 5;
// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
pub const FEATURE_FAILURE_RATE: usize = 6;
// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
pub const FEATURE_DENIAL_RATE: usize = 7;
// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
pub const FEATURE_NET_EGRESS_BYTES: usize = 8;
// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
pub const FEATURE_NET_INGRESS_BYTES: usize = 9;
// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
pub const FEATURE_FORK_RATE: usize = 10;
// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
pub const FEATURE_CHECKPOINT_RATE: usize = 11;
// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
pub const FEATURE_ROLLBACK_RATE: usize = 12;
// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
pub const FEATURE_WATCHDOG_FIRES: usize = 13;
// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
pub const FEATURE_PARKED_CONNECTIONS: usize = 14;
// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
pub const FEATURE_OUTBOX_BYTES: usize = 15;

// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnomalySignal {
    RawIpWithoutDns,
    DenialBurst,
    PrivilegeDrift,
}

// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetryRingConfig {
    pub feature_extraction: bool,
    pub sliding_window_depth: usize,
    pub export_feature_vector: bool,
}

impl Default for TelemetryRingConfig {
    fn default() -> Self {
        Self {
            feature_extraction: true,
            sliding_window_depth: TELEMETRY_RING_DEPTH,
            export_feature_vector: true,
        }
    }
}

// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TelemetrySample {
    pub syscall_rate: f32,
    pub arg_entropy: f32,
    pub burst: f32,
    pub trap_rate: f32,
    pub fs_lock_rate: f32,
    pub exec_stdout_bytes: f32,
    pub failure_rate: f32,
    pub denial_rate: f32,
    pub net_egress_bytes: f32,
    pub net_ingress_bytes: f32,
    pub fork_rate: f32,
    pub checkpoint_rate: f32,
    pub rollback_rate: f32,
    pub watchdog_fires: f32,
    pub parked_connections: f32,
    pub outbox_bytes: f32,
    // see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
    pub raw_ip_without_dns: f32,
    // see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
    pub denial_burst: f32,
    // see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
    pub privilege_drift: f32,
}

impl TelemetrySample {
    // see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
    pub fn flagged_signals(&self) -> Vec<AnomalySignal> {
        let mut signals = Vec::new();
        if self.raw_ip_without_dns > 0.0 {
            signals.push(AnomalySignal::RawIpWithoutDns);
        }
        if self.denial_burst > 0.0 {
            signals.push(AnomalySignal::DenialBurst);
        }
        if self.privilege_drift > 0.0 {
            signals.push(AnomalySignal::PrivilegeDrift);
        }
        signals
    }
    // see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
    pub fn as_array(&self) -> [f32; FEATURE_VECTOR_DIMS] {
        [
            self.syscall_rate,
            self.arg_entropy,
            self.burst,
            self.trap_rate,
            self.fs_lock_rate,
            self.exec_stdout_bytes,
            self.failure_rate,
            self.denial_rate,
            self.net_egress_bytes,
            self.net_ingress_bytes,
            self.fork_rate,
            self.checkpoint_rate,
            self.rollback_rate,
            self.watchdog_fires,
            self.parked_connections,
            self.outbox_bytes,
        ]
    }
}

// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FeatureVector(pub [f32; FEATURE_VECTOR_DIMS]);

impl FeatureVector {
    // see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
    pub fn normalized(raw: [f32; FEATURE_VECTOR_DIMS], scales: [f32; FEATURE_VECTOR_DIMS]) -> Self {
        let mut out = [0.0; FEATURE_VECTOR_DIMS];
        for index in 0..FEATURE_VECTOR_DIMS {
            let scale = scales[index];
            let value = if scale > 0.0 { raw[index] / scale } else { 0.0 };
            out[index] = value.clamp(0.0, 1.0);
        }
        Self(out)
    }
}

// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
pub struct TelemetryRing {
    config: TelemetryRingConfig,
    samples: VecDeque<TelemetrySample>,
}

impl TelemetryRing {
    // see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
    pub fn new(config: TelemetryRingConfig) -> Self {
        Self {
            config,
            samples: VecDeque::new(),
        }
    }

    // see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
    pub fn config(&self) -> &TelemetryRingConfig {
        &self.config
    }

    // see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    // see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    // see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
    pub fn push(&mut self, sample: TelemetrySample) {
        if !self.config.feature_extraction {
            return;
        }
        while self.samples.len() >= self.config.sliding_window_depth {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }

    // see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
    pub fn feature_vector(&self, scales: [f32; FEATURE_VECTOR_DIMS]) -> FeatureVector {
        if self.samples.is_empty() {
            return FeatureVector::default();
        }
        let mut sums = [0.0; FEATURE_VECTOR_DIMS];
        for sample in &self.samples {
            let values = sample.as_array();
            for index in 0..FEATURE_VECTOR_DIMS {
                sums[index] += values[index];
            }
        }
        let count = self.samples.len() as f32;
        let mut means = [0.0; FEATURE_VECTOR_DIMS];
        for index in 0..FEATURE_VECTOR_DIMS {
            means[index] = sums[index] / count;
        }
        FeatureVector::normalized(means, scales)
    }
}

impl Default for TelemetryRing {
    fn default() -> Self {
        Self::new(TelemetryRingConfig::default())
    }
}

// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
pub fn otlp_export_payload(service_name: &str, vector: &FeatureVector) -> serde_json::Value {
    serde_json::json!({
        "resourceSpans": [{
            "resource": {"attributes": [{"key": "service.name", "value": {"stringValue": service_name}}]},
            "scopeSpans": [{
                "spans": [{
                    "name": "kelir.telemetry.features",
                    "attributes": [{"key": "kelir.feature_vector", "value": {"stringValue": format!("{:?}", vector.0)}}],
                }]
            }]
        }]
    })
}

// see: cans/plugins/anomaly.md#Error-and-return-semantics-(anomaly)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnomalyError {
    Eio(String),
}

impl AnomalyError {
    // see: cans/plugins/anomaly.md#Error-and-return-semantics-(anomaly)
    pub fn reason(&self) -> &str {
        match self {
            Self::Eio(reason) => reason,
        }
    }
}

// see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurprisalEscalation {
    pub feature_vector: FeatureVector,
}

// see: cans/plugins/anomaly.md#Service-exports-(anomaly)
#[derive(Debug, Clone, PartialEq)]
pub struct AnomalyOutcome {
    pub feature_vector: FeatureVector,
    pub surprisal: f32,
    pub escalate: bool,
    // see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
    pub signals: Vec<AnomalySignal>,
}

// see: cans/plugins/anomaly.md#Service-exports-(anomaly)
pub struct AnomalyDetectionService {
    ring: Mutex<TelemetryRing>,
    threshold: f32,
}

impl AnomalyDetectionService {
    // see: cans/plugins/anomaly.md#Service-exports-(anomaly)
    pub fn new(config: TelemetryRingConfig, threshold: f32) -> Self {
        Self {
            ring: Mutex::new(TelemetryRing::new(config)),
            threshold,
        }
    }

    // see: cans/plugins/anomaly.md#Service-exports-(anomaly)
    pub fn observe(
        &self,
        sample: TelemetrySample,
        scales: [f32; FEATURE_VECTOR_DIMS],
    ) -> Result<AnomalyOutcome, AnomalyError> {
        let mut ring = self
            .ring
            .lock()
            .map_err(|_| AnomalyError::Eio("anomaly ring lock poisoned".into()))?;
        let signals = sample.flagged_signals();
        ring.push(sample);
        let vector = ring.feature_vector(scales);
        let surprisal = self.surprisal(&vector);
        let escalate = surprisal > self.threshold || !signals.is_empty();
        Ok(AnomalyOutcome {
            feature_vector: vector,
            surprisal,
            escalate,
            signals,
        })
    }

    // TODO (L3) [telemetry_audit]: replace max-component scoring with the statistical surprisal model; slop: scoring stays max-component; subtle anomalies pass as normal.
    // see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
    pub fn surprisal(&self, vector: &FeatureVector) -> f32 {
        vector
            .0
            .iter()
            .fold(0.0, |max, value| if *value > max { *value } else { max })
    }

    // see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
    pub fn escalate(&self, bus: &EventBus, outcome: &AnomalyOutcome) {
        if outcome.escalate {
            bus.publish(&SurprisalEscalation {
                feature_vector: outcome.feature_vector,
            });
        }
    }
}

impl Default for AnomalyConfig {
    fn default() -> Self {
        Self {
            sliding_window_depth: TELEMETRY_RING_DEPTH,
            export_feature_vector: true,
            feature_extraction: true,
            surprisal_threshold: 1.0,
        }
    }
}

impl AnomalyConfig {
    // see: cans/plugins/anomaly.md#Invariants-and-policy-(anomaly)
    pub fn ring_config(&self) -> TelemetryRingConfig {
        TelemetryRingConfig {
            feature_extraction: self.feature_extraction,
            sliding_window_depth: self.sliding_window_depth,
            export_feature_vector: self.export_feature_vector,
        }
    }
}

impl AnomalyCtx {
    // see: cans/plugins/anomaly.md#Service-exports-(anomaly)
    pub fn service(&self) -> &Arc<AnomalyDetectionService> {
        &self.service
    }

    // see: cans/plugins/anomaly.md#Service-exports-(anomaly)
    pub fn observe(
        &self,
        sample: TelemetrySample,
        scales: [f32; FEATURE_VECTOR_DIMS],
    ) -> Result<AnomalyOutcome, AnomalyError> {
        self.service.observe(sample, scales)
    }
}

impl AnomalyPlugin {
    // see: cans/plugins/anomaly.md#Plugin-anomaly
    pub fn new(config: AnomalyConfig) -> Self {
        let threshold = config.surprisal_threshold;
        let service = Arc::new(AnomalyDetectionService::new(
            config.ring_config(),
            threshold,
        ));
        Self { config, service }
    }

    // see: cans/plugins/anomaly.md#Plugin-anomaly
    pub fn with_config(mut self, config: AnomalyConfig) -> Self {
        self.service = Arc::new(AnomalyDetectionService::new(
            config.ring_config(),
            config.surprisal_threshold,
        ));
        self.config = config;
        self
    }

    // see: cans/plugins/anomaly.md#Service-exports-(anomaly)
    pub fn service(&self) -> &Arc<AnomalyDetectionService> {
        &self.service
    }
}

impl Default for AnomalyPlugin {
    fn default() -> Self {
        Self::new(AnomalyConfig::default())
    }
}

impl KelirPlugin for AnomalyPlugin {
    type Context = AnomalyCtx;
    fn context(&self) -> AnomalyCtx {
        AnomalyCtx {
            service: Arc::clone(&self.service),
        }
    }
    fn name(&self) -> &'static str {
        "anomaly"
    }
}
