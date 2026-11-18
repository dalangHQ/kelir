// see: cans/plugins/anomaly.md#Plugin-anomaly
#![forbid(unsafe_code)]

use std::sync::Arc;

use kelir_interface::plugin_model::{KelirPlugin, LifecycleParticipant, PluginDeps};

#[path = "service_anomaly.flow.rs"]
pub mod service_anomaly;

pub use service_anomaly::{
    AnomalyDetectionService, AnomalyError, AnomalyOutcome, FEATURE_ARG_ENTROPY, FEATURE_BURST,
    FEATURE_CHECKPOINT_RATE, FEATURE_DENIAL_RATE, FEATURE_EXEC_STDOUT_BYTES, FEATURE_FAILURE_RATE,
    FEATURE_FORK_RATE, FEATURE_FS_LOCK_RATE, FEATURE_NET_EGRESS_BYTES, FEATURE_NET_INGRESS_BYTES,
    FEATURE_OUTBOX_BYTES, FEATURE_PARKED_CONNECTIONS, FEATURE_ROLLBACK_RATE, FEATURE_SYSCALL_RATE,
    FEATURE_TRAP_RATE, FEATURE_VECTOR_DIMS, FEATURE_WATCHDOG_FIRES, FeatureVector,
    SurprisalEscalation, TELEMETRY_RING_DEPTH, TelemetryRing, TelemetryRingConfig, TelemetrySample,
    otlp_export_payload,
};

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.anomaly
#[derive(Debug, Clone, PartialEq)]
pub struct AnomalyConfig {
    pub sliding_window_depth: usize,
    pub export_feature_vector: bool,
    pub feature_extraction: bool,
    pub surprisal_threshold: f32,
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

// see: cans/plugins/anomaly.md#Service-exports-(anomaly)
#[derive(Clone)]
pub struct AnomalyCtx {
    service: Arc<AnomalyDetectionService>,
}

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
// see: cans/plugins/anomaly.md#Service-exports-(anomaly)
pub type AnomalyHandle = AnomalyCtx;

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

// see: cans/plugins/anomaly.md#Plugin-anomaly
#[derive(Clone, PluginDeps)]
pub struct AnomalyPlugin {
    pub config: AnomalyConfig,
    service: Arc<AnomalyDetectionService>,
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

impl LifecycleParticipant for AnomalyPlugin {}

// see: cans/02-plugin-model.md#KelirPlugin-trait
