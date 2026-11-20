// see: cans/plugins/anomaly.md#Plugin-anomaly
#![forbid(unsafe_code)]

use std::sync::Arc;

use kelir_interface::plugin_model::PluginDeps;

#[path = "service_anomaly.flow.rs"]
pub mod service_anomaly;
#[path = "service_anomaly.run.rs"]
pub mod service_anomaly_run;

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

// see: cans/plugins/anomaly.md#Service-exports-(anomaly)
#[derive(Clone)]
pub struct AnomalyCtx {
    service: Arc<AnomalyDetectionService>,
}

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
// see: cans/plugins/anomaly.md#Service-exports-(anomaly)
pub type AnomalyHandle = AnomalyCtx;

// see: cans/plugins/anomaly.md#Plugin-anomaly
#[derive(Clone, PluginDeps)]
pub struct AnomalyPlugin {
    pub config: AnomalyConfig,
    service: Arc<AnomalyDetectionService>,
}

// see: cans/02-plugin-model.md#KelirPlugin-trait
