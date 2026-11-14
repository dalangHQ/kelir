// Config structs derive Default; the builder sets fields via typestate setters.
// Every field name below is copied from the live definitions:
//   VmConfig/DriverPolicy: artifacts/code/vmm-driver-trait.rs
//   StoragePlaneConfig: crates/kelir/src/planes/storage_plane.rs
//   WireConfig/SinkholeConfig: crates/kelir/src/planes/wire_plane.rs
//   PtyConfig: crates/kelir/src/planes/terminal_plane.flow.rs
//   ContainerConfig: crates/kelir-plugins/plugin-container/src/lib.rs
// Default values follow the Embedded-profile preset where one exists
// (1 vCPU, 256MB RAM); adding a field is not a breaking change to callers.
//
// Owned by: cans/01-architecture.md#Codebase-invariants

#![forbid(unsafe_code)]

use std::path::PathBuf;

// see: cans/01-architecture.md#Driver-backends
#[derive(Debug, Clone, Default)]
pub enum WxPolicy {
    /// Hard panic on anonymous executable mappings or dynamic mprotect(PROT_EXEC).
    #[default]
    Strict,
    /// Log violation to host ledger and continue execution.
    PermissiveAudit,
}

/// see: cans/01-architecture.md#Fallback-policy
#[derive(Debug, Clone, Default)]
pub struct DriverPolicy {
    pub driver: Option<String>,
    pub require_hardware_vmm: bool,
    pub allow_emulated_fallback: bool,
    pub verify_hw_lockfile: bool,
    pub wx_enforcement: WxPolicy,
    pub ffi_worker_threads: usize,
    pub ffi_max_ops_per_sec: u32,
}

// see: cans/01-architecture.md#VmmDriver-trait
#[derive(Debug, Clone, Default)]
pub struct VmConfig {
    pub vcpus: u8,
    pub memory_mb: u32,
    pub kernel_path: PathBuf,
    pub rootfs_erofs_path: PathBuf,
    pub page_size_kb: u32,
    pub tsc_locked: bool,
    pub entropy_seed: Option<u64>,
    pub driver_policy: DriverPolicy,
    pub enable_gpu_passthrough: bool,
    pub enable_watchdog: bool,
    pub deterministic_clock: bool,
    pub pinned_clocksource: Option<String>,
    pub ns_per_instruction: Option<u32>,
    pub preemption_grace_ms: u64,
    pub network_off: bool,
}

// see: cans/05-storage-plane.md#Virtual-ID-remapping
#[derive(Debug, Clone, Copy, Default)]
pub struct IdmapConfig {
    pub guest_uid: u32,
    pub host_uid: u32,
}

// see: cans/05-storage-plane.md#In-flight-read-masking
#[derive(Debug, Clone, Copy, Default)]
pub enum ReadMaskEngine {
    #[default]
    FastRegex,
}

// see: cans/05-storage-plane.md#Storage-architecture
#[derive(Debug, Clone, Copy, Default)]
pub enum WorkspaceEncapsulation {
    #[default]
    Passthrough,
}

/// see: cans/05-storage-plane.md#Storage-architecture
#[derive(Debug, Clone, Default)]
pub struct StoragePlaneConfig {
    pub idmap: IdmapConfig,
    pub shared_caches: Vec<String>,
    pub lock_broker: bool,
    pub synthetic_files: Vec<SyntheticFile>,
    pub mask_read: Vec<MaskReadRule>,
    pub encapsulation: WorkspaceEncapsulation,
    pub scan_engine: ReadMaskEngine,
    pub carryover_bytes: usize,
}

// see: cans/04-terminal-plane.md#Terminal-pipeline
#[derive(Debug, Clone, Default)]
pub struct TokenShieldConfig {
    pub max_bytes_total: usize,
    pub max_lines_total: usize,
    pub clamp_strategy: String,
    pub notice: String,
}

/// see: cans/04-terminal-plane.md#Terminal-pipeline
#[derive(Debug, Clone, Default)]
pub struct PtyConfig {
    pub cols: u16,
    pub rows: u16,
    pub drop_osc52: bool,
    pub strip_title: bool,
    pub token_shield: Option<TokenShieldConfig>,
    pub prompts: Vec<PromptRule>,
    pub stream_engine: String,
}

// see: cans/07-wire-plane.md#DNS-policy-filter
#[derive(Debug, Clone, Default)]
pub struct SinkholeConfig {
    pub allowlist: Vec<String>,
    pub drop_all: bool,
}

// see: cans/07-wire-plane.md#L4-diversion
#[derive(Debug, Clone, Default)]
pub struct L4Proxy {
    pub listen_port: u16,
    pub destination: String,
}

// see: cans/07-wire-plane.md#Wire-plane
#[derive(Debug, Clone, Copy, Default)]
pub enum TapFilter {
    #[default]
    AllowUdpProxied,
}

/// see: cans/07-wire-plane.md#Wire-plane
#[derive(Debug, Clone, Default)]
pub struct WireConfig {
    pub sinkhole: SinkholeConfig,
    pub ssh_agent_bridge: bool,
    pub vaults: Vec<CredentialDescriptor>,
    pub l4_proxies: Vec<L4Proxy>,
    pub tap_filter: TapFilter,
}

// Plugin *Config exemplar; every plugin *Config follows the same contract.
/// see: cans/plugins/container.md#Plugin-container
#[derive(Debug, Clone, Default)]
pub struct ContainerConfig {
    pub runtime: String,
    pub isolate_var_lib_containers: bool,
    pub allow_unsandboxed: bool,
    pub pin_subuid_ranges: bool,
    pub scratch_device: Option<PathBuf>,
}

// Supporting types referenced above; their full field sets live at their
// owning artifacts and are not duplicated here.
// see: cans/artifacts/code/vmm-driver-trait.rs
pub use crate::credential_vault::CredentialDescriptor;
// see: cans/05-storage-plane.md#Storage-architecture
pub use crate::storage_plane::{MaskReadRule, SyntheticFile};
// see: cans/04-terminal-plane.md#Terminal-pipeline
pub use crate::terminal_plane::PromptRule;

// see: cans/01-architecture.md#Embedded-profile
// Typestate setters on KelirBuilder replace default_vm_config(...): the
// builder starts from VmConfig::default() and each setter overwrites one
// field, so a new VmConfig field starts at its Default instead of breaking
// every construction site.
// see: cans/02-plugin-model.md#Unified-builder
fn embedded_profile_builder() -> KelirBuilder {
    const ARCH: &str = std::env::consts::ARCH;
    KelirBuilder::new(VmConfig::default())
        .vcpus(4)
        .memory_mb(4096)
        .kernel(format!("./fixtures/vmlinux-6.6-{ARCH}-kelir"))
        .rootfs("./fixtures/rootfs-swebench.erofs")
        .tsc_locked(true)
        .watchdog(true)
        .driver(DriverPolicy::default().wx_enforcement(WxPolicy::Strict))
}
