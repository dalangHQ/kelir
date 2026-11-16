// see: cans/14-observability.md#Telemetry-ring
use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

// see: cans/14-observability.md#Telemetry-ring
pub const TELEMETRY_RING_DEPTH: usize = 300;
// see: cans/14-observability.md#Telemetry-ring
pub const FEATURE_VECTOR_DIMS: usize = 16;
// see: cans/14-observability.md#Telemetry-ring
pub const FEATURE_SYSCALL_RATE: usize = 0;
// see: cans/14-observability.md#Telemetry-ring
pub const FEATURE_ARG_ENTROPY: usize = 1;
// see: cans/14-observability.md#Telemetry-ring
pub const FEATURE_BURST: usize = 2;
// see: cans/14-observability.md#Telemetry-ring
pub const FEATURE_TRAP_RATE: usize = 3;
// see: cans/14-observability.md#Telemetry-ring
pub const FEATURE_FS_LOCK_RATE: usize = 4;
// see: cans/14-observability.md#Telemetry-ring
pub const FEATURE_EXEC_STDOUT_BYTES: usize = 5;
// see: cans/14-observability.md#Telemetry-ring
pub const FEATURE_FAILURE_RATE: usize = 6;
// see: cans/14-observability.md#Telemetry-ring
pub const FEATURE_DENIAL_RATE: usize = 7;
// see: cans/14-observability.md#Telemetry-ring
pub const FEATURE_NET_EGRESS_BYTES: usize = 8;
// see: cans/14-observability.md#Telemetry-ring
pub const FEATURE_NET_INGRESS_BYTES: usize = 9;
// see: cans/14-observability.md#Telemetry-ring
pub const FEATURE_FORK_RATE: usize = 10;
// see: cans/14-observability.md#Telemetry-ring
pub const FEATURE_CHECKPOINT_RATE: usize = 11;
// see: cans/14-observability.md#Telemetry-ring
pub const FEATURE_ROLLBACK_RATE: usize = 12;
// see: cans/14-observability.md#Telemetry-ring
pub const FEATURE_WATCHDOG_FIRES: usize = 13;
// see: cans/14-observability.md#Telemetry-ring
pub const FEATURE_PARKED_CONNECTIONS: usize = 14;
// see: cans/14-observability.md#Telemetry-ring
pub const FEATURE_OUTBOX_BYTES: usize = 15;

// see: cans/14-observability.md#Telemetry-ring
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

// see: cans/14-observability.md#Telemetry-ring
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
}

impl TelemetrySample {
    // see: cans/14-observability.md#Telemetry-ring
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

// see: cans/14-observability.md#Telemetry-ring
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FeatureVector(pub [f32; FEATURE_VECTOR_DIMS]);

impl FeatureVector {
    // see: cans/14-observability.md#Telemetry-ring
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

// see: cans/14-observability.md#Telemetry-ring
pub struct TelemetryRing {
    config: TelemetryRingConfig,
    samples: VecDeque<TelemetrySample>,
}

impl TelemetryRing {
    // see: cans/14-observability.md#Telemetry-ring
    pub fn new(config: TelemetryRingConfig) -> Self {
        Self {
            config,
            samples: VecDeque::new(),
        }
    }

    // see: cans/14-observability.md#Telemetry-ring
    pub fn config(&self) -> &TelemetryRingConfig {
        &self.config
    }

    // see: cans/14-observability.md#Telemetry-ring
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    // see: cans/14-observability.md#Telemetry-ring
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    // see: cans/14-observability.md#Telemetry-ring
    pub fn push(&mut self, sample: TelemetrySample) {
        if !self.config.feature_extraction {
            return;
        }
        while self.samples.len() >= self.config.sliding_window_depth {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }

    // see: cans/14-observability.md#Telemetry-ring
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

// see: cans/14-observability.md#Telemetry-ring
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtlpExporterConfig {
    pub endpoint: String,
    pub service_name: String,
}

impl Default for OtlpExporterConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:4318/v1/traces".to_string(),
            service_name: "kelir".to_string(),
        }
    }
}

// see: cans/14-observability.md#Telemetry-ring
pub fn otlp_export_payload(
    config: &OtlpExporterConfig,
    vector: &FeatureVector,
) -> serde_json::Value {
    serde_json::json!({
        "resourceSpans": [{
            "resource": {"attributes": [{"key": "service.name", "value": {"stringValue": config.service_name}}]},
            "scopeSpans": [{
                "spans": [{
                    "name": "kelir.telemetry.features",
                    "attributes": [{"key": "kelir.feature_vector", "value": {"stringValue": format!("{:?}", vector.0)}}],
                }]
            }]
        }]
    })
}
