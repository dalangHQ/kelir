// see: cans/plugins/profile.md#Session-compute-schema-(profile)
// see: cans/plugins/profile.md#Inversion-of-Control-configuration-router
use kelir_catalog::CatalogBackend;
use kelir_interface::driver_interface::{DriverPolicy, FastBootMode, VmConfig, WxPolicy};
use serde::{Deserialize, Serialize};

// see: cans/artifacts/examples/fixtures/kelir.yaml#invariants.physicality_check
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhysicalityCheck {
    #[serde(rename = "strict")]
    Strict,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#invariants.inheritance_lattice
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InheritanceLattice {
    #[serde(rename = "monotonic_with_signed_operator_lease")]
    MonotonicWithSignedOperatorLease,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#invariants.memory_safety
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemorySafety {
    #[serde(rename = "forbid_unsafe")]
    ForbidUnsafe,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#invariants.policy_plane_mutability
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyPlaneMutability {
    #[serde(rename = "dynamic_signed")]
    DynamicSigned,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#invariants
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvariantsProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_zero_capability: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physicality_check: Option<PhysicalityCheck>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inheritance_lattice: Option<InheritanceLattice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_safety: Option<MemorySafety>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_plane_mutability: Option<PolicyPlaneMutability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_lease: Option<OperatorLeaseProfile>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#invariants.operator_lease
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorLeaseProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify_ed25519_pubkey: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_runtime_escalation: Option<bool>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#driver.emulated_tier_classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmulatedTierClassification {
    #[serde(rename = "Class2_RootlessUserNS")]
    Class2RootlessUserNS,
}

impl EmulatedTierClassification {
    pub fn as_str(&self) -> &'static str {
        match self {
            EmulatedTierClassification::Class2RootlessUserNS => "Class2_RootlessUserNS",
        }
    }
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#driver.ffi_boundary
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmbedMode {
    #[serde(rename = "in_process_libkelir")]
    InProcessLibkelir,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#driver.ffi_boundary
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FfiBoundaryProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embed_mode: Option<EmbedMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_threads: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_ops_per_second: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue_capacity: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_buffer_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zero_copy_shm_threshold_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_async_worker_yield: Option<bool>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#driver
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriverProfileTop {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_hardware_vmm: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_emulated_fallback: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emulated_tier_classification: Option<EmulatedTierClassification>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify_hw_lockfile: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wx_enforcement: Option<WxPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast_boot: Option<FastBootMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ffi_boundary: Option<FfiBoundaryProfile>,
}

// see: cans/plugins/profile.md#Session-compute-schema-(profile)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topology: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isolation_envelope: Option<IsolationEnvelopeProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compute: Option<ComputeProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balloon: Option<BalloonProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock: Option<ClockProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watchdog: Option<WatchdogProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub devices: Option<DevicesProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<ArtifactsProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shim: Option<ShimProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver: Option<DriverProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<StorageProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<BudgetProfile>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#session.isolation_envelope
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IsolationEnvelopeProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_openat2_beneath: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cgroup_parent: Option<String>,
}

// see: cans/plugins/profile.md#Session-compute-schema-(profile)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exec_seconds: Option<u64>,
}

// see: cans/plugins/profile.md#Session-compute-schema-(profile)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComputeProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcpus: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_vcpus: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_mb: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_size_kb: Option<u32>,
}

// see: cans/plugins/profile.md#Session-compute-schema-(profile)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BalloonProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_mb: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_reclaim: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reclaim_strategy: Option<String>,
}

// see: cans/plugins/profile.md#Session-compute-schema-(profile)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactsProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kernel_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rootfs_erofs_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kernel_boot_params: Option<Vec<String>>,
}

// see: cans/plugins/profile.md#Session-compute-schema-(profile)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClockProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tsc_locked: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deterministic_clock: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_clocksource: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preemption_grace_ms: Option<u64>,
}

// see: cans/plugins/profile.md#Session-compute-schema-(profile)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WatchdogProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heartbeat_cadence_seconds: Option<u64>,
}

// see: cans/plugins/profile.md#Session-compute-schema-(profile)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DevicesProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub virtio_rng: Option<VirtioRngProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub virtio_shm: Option<VirtioShmProfile>,
}

// see: cans/plugins/profile.md#Session-compute-schema-(profile)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VirtioRngProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

// see: cans/plugins/profile.md#Session-compute-schema-(profile)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VirtioShmProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regions: Option<Vec<VirtioShmRegionProfile>>,
}

// see: cans/plugins/profile.md#Session-compute-schema-(profile)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VirtioShmRegionProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#session.shim.rejection_mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectionMode {
    #[serde(rename = "synthetic_refusal")]
    SyntheticRefusal,
}

// see: cans/plugins/profile.md#Session-compute-schema-(profile)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShimProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_exec_slots: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_pid: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anti_toctou_cgroups: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub granular_task_freeze: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_freeze_timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejection_mode: Option<RejectionMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_proc_environ: Option<bool>,
}

// see: cans/plugins/profile.md#Session-compute-schema-(profile)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriverProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast_boot: Option<String>,
}

// see: cans/plugins/profile.md#Session-compute-schema-(profile)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dax_window: Option<String>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#storage
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageTopProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dax_window: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cow_broker: Option<CowBrokerProfile>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#storage.cow_broker
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CowBrokerProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sparse_upper: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intercept_vfs_mutations: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_path_redirection: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tombstone_tracking: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback_purge: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir_merge_cache: Option<bool>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#wire.outbox_deferral
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutboxDeferralProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buffer_capacity_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_on_step_verification: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drop_on_rollback: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compensator_webhook_timeout_ms: Option<u64>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#wire
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub userspace_netstack: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_egress: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub l4_diversion: Option<L4DiversionProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub l7_protocol_inspection: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outbox_deferral: Option<OutboxDeferralProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_abstract_unix_sockets: Option<bool>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#wire.l4_diversion
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct L4DiversionProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxies: Option<Vec<L4DiversionProxyProfile>>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#wire.l4_diversion.proxies
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct L4DiversionProxyProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forward_unix_socket: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inject_mtls: Option<L4DiversionMtlsProfile>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#wire.l4_diversion.proxies.inject_mtls
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct L4DiversionMtlsProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cert_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_path: Option<String>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#checkpoint.step_dag.storage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepDagStorage {
    #[serde(rename = "content_addressed")]
    ContentAddressed,
}

impl StepDagStorage {
    // see: cans/artifacts/examples/fixtures/kelir.yaml#checkpoint.step_dag.storage
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ContentAddressed => "content_addressed",
        }
    }
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#checkpoint.step_dag
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepDagProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_signed_attribution: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_separation_of_duties: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<StepDagStorage>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#checkpoint
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_dag: Option<StepDagProfile>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#host.catalog
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<CatalogBackend>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_tenant_partitioning: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_sqlite_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wal_mode: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub busy_timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_ttl_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prune_unreferenced_cas_blobs: Option<bool>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#host.telemetry_ring
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TelemetryRingProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature_extraction: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sliding_window_depth: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_feature_vector: Option<bool>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#host
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog: Option<CatalogProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telemetry_ring: Option<TelemetryRingProfile>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.hashing_standard
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HashingStandard {
    #[serde(rename = "RFC_6962_DUAL_BLAKE3_SHA256")]
    Rfc6962DualBlake3Sha256,
}

impl HashingStandard {
    pub fn as_str(&self) -> &'static str {
        match self {
            HashingStandard::Rfc6962DualBlake3Sha256 => "RFC_6962_DUAL_BLAKE3_SHA256",
        }
    }
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MerkleWalProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hashing_standard: Option<HashingStandard>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sign_leaves: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fsync_on_reap: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_pre_redaction_hash: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_hook_spans_in_root: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub witness_cosigning_endpoint: Option<String>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.otel
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OtelProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub propagate_w3c_traceparent: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_diagnostic_pmu_gauges: Option<bool>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerPluginProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merkle_wal: Option<MerkleWalProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub otel: Option<OtelProfile>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.tool_guard.mcp_interposition
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpInterpositionProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin_schemas: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub halt_on_schema_drift: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detect_semantic_rug_pull: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict_ast_parameter_validation: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_invocations_per_trace: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_frame_bytes: Option<u64>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.tool_guard.taint_governance
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaintGovernanceProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub untrusted_ingress_taints: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub propagate_byte_range_taints: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mutating_verbs_require_approval: Option<bool>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.tool_guard
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolGuardPluginProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_interposition: Option<McpInterpositionProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taint_governance: Option<TaintGovernanceProfile>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.mcp
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpPluginProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socket_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub framing: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_frame_bytes: Option<u64>,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginsProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ledger: Option<LedgerPluginProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_guard: Option<ToolGuardPluginProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp: Option<McpPluginProfile>,
    #[serde(default, flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// see: cans/plugins/profile.md#Session-compute-schema-(profile)
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EffectiveConfig {
    pub policy_plane_mutability: Option<PolicyPlaneMutability>,
    pub inheritance_lattice: Option<InheritanceLattice>,
    pub operator_lease: Option<OperatorLeaseProfile>,
    pub embed_mode: Option<EmbedMode>,
    pub driver_policy: DriverPolicy,
    pub vm_config: VmConfig,
    pub tenant_id: Option<String>,
    pub org_id: Option<String>,
    pub isolation_envelope: Option<IsolationEnvelopeProfile>,
    pub granular_task_freeze: Option<bool>,
    pub task_freeze_timeout_ms: Option<u64>,
    pub rejection_mode: Option<RejectionMode>,
    pub cow_broker: Option<CowBrokerProfile>,
    pub userspace_netstack: Option<bool>,
    pub default_egress: Option<String>,
    pub l4_diversion: Option<L4DiversionProfile>,
    pub l7_protocol_inspection: Option<bool>,
    pub outbox_deferral: Option<OutboxDeferralProfile>,
    pub step_dag: Option<StepDagProfile>,
    pub catalog: Option<CatalogProfile>,
    pub telemetry_ring: Option<TelemetryRingProfile>,
    pub ledger_merkle_wal: Option<MerkleWalProfile>,
    pub ledger_otel: Option<OtelProfile>,
    pub tool_guard_interposition: Option<McpInterpositionProfile>,
    pub tool_guard_taint: Option<TaintGovernanceProfile>,
    pub mcp: Option<McpPluginProfile>,
}
