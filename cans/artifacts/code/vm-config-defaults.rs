// Config structs derive Default; the builder sets fields via typestate setters.
// Every field name below is copied from the live definitions:
//   VmConfig/DriverPolicy: artifacts/code/vmm-driver-trait.rs
//   StoragePlaneConfig: crates/kelir/src/planes/storage_plane.rs
//   WireConfig/SinkholeConfig: crates/kelir/src/planes/wire_plane.rs
//   PtyConfig: crates/kelir/src/planes/terminal_plane.flow.rs
//   ContainerConfig: crates/kelir-plugins/plugin-container/src/lib.rs
// Extracted fields are marked inline with their plugin destination:
// Part 1/7 (wire), Part 2/7 (terminal), Part 5/7 (storage). Plugin
// *Config types are the plugin-side pins; core structs carry the
// primitive surface only.
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
    /// Masks /proc/[pid]/environ and process_vm_readv in-guest via seccomp.
    pub mask_proc_environ: bool,
    /// Leaf cgroup v2 isolates pids.max and memory.max per ExecSlot
    /// (11-protocol.md#Exec-slot-slab); shared-UID RLIMIT_NPROC alone
    /// lets one ExecSlot starve siblings.
    pub per_exec_cgroup: bool,
    /// Syscall trap validates native AUDIT_ARCH; x32 and compat ABI
    /// syscall numbers reject fail-closed (01-architecture.md).
    pub deny_abi_compat: bool,
    /// Guest BPF blocks io_uring, add_key, request_key, keyctl by default
    /// (01-architecture.md#Guest-kernel-hardening).
    pub blocked_syscalls: Vec<String>,
    /// Exotic socket families denied by default: AF_ALG, AF_RXRPC,
    /// NETLINK_XFRM (01-architecture.md#Guest-kernel-hardening).
    pub blocked_socket_families: Vec<String>,
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
    /// flock bridge: core-owned per 05-storage-plane (Part 5/7).
    pub lock_broker: bool,
    /// openat2 containment: core-owned per 05-storage-plane (Part 5/7).
    pub encapsulation: WorkspaceEncapsulation,
    // Extracted (Part 5/7):
    //   synthetic_files  → plugins.vault.synthetic_files / plugins.profile.synthetic_files
    //   mask_read        → plugins.redaction.mask_read.rules
    //   scan_engine      → plugins.redaction.mask_read.engine
    //   carryover_bytes  → plugins.redaction.mask_read.carryover_bytes
}

/// see: cans/04-terminal-plane.md#Terminal-pipeline
#[derive(Debug, Clone, Default)]
pub struct PtyConfig {
    pub cols: u16,
    pub rows: u16,
    // TO CONFIRM: Part 2/7 states core terminal stages are "raw VT100
    // grid, byte-stream ring, index.bin replay only". If ESC sanitization
    // follows that rule, drop_osc52 and strip_title move to
    // plugin-terminal-guard. They are not explicitly named in the
    // Part 2/7 removal list, so they stay here pending confirmation.
    pub drop_osc52: bool,
    pub strip_title: bool,
    pub stream_engine: String,
    // Extracted (Part 2/7):
    //   token_shield (incl. its notice: String) → plugins.token_shield
    //   prompts                                 → plugins.terminal_guard.prompts
    //   terminal.sanitizer.notice (ANSI box)    → plugins.terminal_guard.notice_template
}

// see: cans/07-wire-plane.md#DNS-policy-filter
// Canonical shape: rules: Vec<SinkholeRule>; allowlist / drop_all are
// sugar desugared by SinkholeConfig::from_allowlist
// (see: artifacts/code/sinkhole-rules.rs). SinkholeRule carries
// { host, allow }; methods / paths removed (Part 1/7).
#[derive(Debug, Clone, Default)]
pub struct SinkholeConfig {
    pub rules: Vec<SinkholeRule>,
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
    /// Blocks in-guest AF_UNIX sockets with leading null bytes (abstract sockets).
    pub block_abstract_unix_sockets: bool,
    /// Correlates allowed domain answers to TTL-bounded L4 IP pins;
    /// direct IP dials absent from the table drop fail-closed
    /// (07-wire-plane.md#DNS-to-IP-admission).
    pub dns_to_ip_pinning: bool,
    pub sinkhole: SinkholeConfig,
    pub ssh_agent_bridge: bool,
    /// Interposition is plugin-vault (Part 1/7); this field declares
    /// the vaults the plugin mounts. Retained pending confirmation.
    pub vaults: Vec<CredentialDescriptor>,
    pub tap_filter: TapFilter,
    // Extracted (Part 1/7):
    //   l4_proxies → plugins.egress_proxy.upstream_dial
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
// see: cans/artifacts/code/sinkhole-rules.rs (canonical shape)
pub use kelir_interface::wire_plane::sinkhole_rules::SinkholeRule;
// Extracted type re-exports removed (Parts 1/2/5):
//   L4Proxy          → plugin-egress-proxy
//   TokenShieldConfig→ plugin-token-shield
//   ReadMaskEngine   → plugin-redaction (private; FastRegex | StreamingAutomata)
//   MaskReadRule     → plugin-redaction
//   SyntheticFile    → plugin-vault / plugin-profile
//   PromptRule       → plugin-terminal-guard

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
