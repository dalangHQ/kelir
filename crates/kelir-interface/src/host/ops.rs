// see: cans/01-architecture.md#Host-plane-subsystem
// see: cans/01-architecture.md#Host-memory-pressure-watcher
// see: cans/08-lifecycle.md#Host-crash-reaper
use serde::{Deserialize, Serialize};

pub enum HostReaperKind {
    LinuxPdeathsig,
    DarwinKqueue,
    WindowsJobObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureSource {
    LinuxCgroupV2MemoryPressure,
    MacosDispatchMemoryPressure,
    WindowsJobObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureLevel {
    None,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PressureEvent {
    pub level: PressureLevel,
    pub at_ns: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PressureSubscription {
    pub source: PressureSource,
    pub armed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BalloonRequest {
    pub target_mb: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReclaimRequest {
    pub bytes: u64,
}

// see: cans/08-lifecycle.md#Host-crash-reaper
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostReaperConfig {
    pub linux_death_signal: String,
    pub darwin_kqueue_event: String,
    pub windows_job_object: bool,
}

// see: cans/01-architecture.md#Host-memory-pressure-watcher
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CgroupConfig {
    pub path: String,
    pub monitor_pressure: bool,
    pub auto_inflate_balloon: bool,
}

// see: cans/06-storage-catalog.md#Entity-model
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CatalogConfig {
    pub backend: String,
    pub endpoint: String,
    pub schema_tenant_partitioning: bool,
    pub fallback_sqlite_path: String,
    pub wal_mode: bool,
    pub busy_timeout_ms: u64,
    pub lease_ttl_seconds: u64,
    pub prune_unreferenced_cas_blobs: bool,
}

// see: cans/14-observability.md#Online-statistical-feature-extraction
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TelemetryRingConfig {
    pub feature_extraction: bool,
    pub sliding_window_depth: u32,
    pub export_feature_vector: bool,
}

// see: cans/01-architecture.md#Host-plane-subsystem
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostPlaneConfig {
    pub reaper: HostReaperConfig,
    pub cgroups: CgroupConfig,
    pub catalog: CatalogConfig,
    pub telemetry_ring: TelemetryRingConfig,
}
