// see: cans/01-architecture.md#VmmDriver-trait
// see: cans/08-lifecycle.md#Hypervisor-primitives
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::AsyncRead;

// see: cans/artifacts/code/vmm-driver-trait.rs#VmmError
#[derive(Error, Debug)]
pub enum VmmError {
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("invalid state: {0}")]
    InvalidState(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("exhausted: {0}")]
    ResourceExhausted(String),
    // see: cans/01-architecture.md#Fallback-policy
    #[error("no hardware virtualization: {0}")]
    NoHardwareVmm(String),
}

pub type DriverError = VmmError;

// see: cans/01-architecture.md#Boot-budget
// see: cans/08-lifecycle.md#Rollback
// see: cans/08-lifecycle.md#CoW-fork
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriverCapabilities {
    pub backend: &'static str,
    pub cold_boot_budget_ms: u64,
    pub rollback_budget_ms: u64,
    pub fork_budget_ms: u64,
    pub requires_hardware_vmm: bool,
    pub supports_cow_fork: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum WxPolicy {
    #[default]
    Strict,
    PermissiveAudit,
}

// see: cans/01-architecture.md#Embedded-profile
#[derive(Debug, Clone, PartialEq)]
pub struct DriverPolicy {
    // see: cans/01-architecture.md#Driver-backends
    pub driver: Option<String>,
    pub require_hardware_vmm: bool,
    pub allow_emulated_fallback: bool,
    pub verify_hw_lockfile: bool,
    pub wx_enforcement: WxPolicy,
    pub ffi_worker_threads: usize,
    pub ffi_max_ops_per_sec: u32,
    // see: cans/artifacts/examples/fixtures/kelir.yaml#driver.ffi_boundary.queue_capacity
    pub ffi_queue_capacity: usize,
    // see: cans/artifacts/examples/fixtures/kelir.yaml#driver.ffi_boundary.max_buffer_bytes
    pub ffi_max_buffer_bytes: usize,
    // see: cans/artifacts/examples/fixtures/kelir.yaml#driver.ffi_boundary.zero_copy_shm_threshold_bytes
    pub ffi_zero_copy_shm_threshold_bytes: usize,
    // see: cans/artifacts/examples/fixtures/kelir.yaml#driver.ffi_boundary.allow_async_worker_yield
    pub ffi_allow_async_worker_yield: bool,
    // see: cans/artifacts/examples/fixtures/kelir.yaml#driver.emulated_tier_classification
    pub emulated_tier_classification: Option<String>,
}

// see: cans/01-architecture.md#Embedded-profile
impl Default for DriverPolicy {
    fn default() -> Self {
        Self {
            driver: None,
            require_hardware_vmm: true,
            allow_emulated_fallback: false,
            verify_hw_lockfile: false,
            wx_enforcement: WxPolicy::Strict,
            // see: cans/01-architecture.md#Foreign-runtime-embedding
            ffi_worker_threads: 8,
            ffi_max_ops_per_sec: 0,
            ffi_queue_capacity: 16384,
            ffi_max_buffer_bytes: 16_777_216,
            ffi_zero_copy_shm_threshold_bytes: 65_536,
            ffi_allow_async_worker_yield: true,
            emulated_tier_classification: None,
        }
    }
}

// see: cans/11-protocol.md#Binary-exec-channel
pub const MSG_EXEC_REFUSAL: &str = "MSG_EXEC_REFUSAL";

// see: cans/11-protocol.md#Binary-exec-channel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RejectionMode {
    #[default]
    SyntheticRefusal,
    Abort,
}

// see: cans/11-protocol.md#Binary-exec-channel
pub fn synthetic_refusal_body(reason: &str) -> String {
    format!("{{\"msg\":\"{MSG_EXEC_REFUSAL}\",\"reason\":\"{reason}\"}}")
}

// see: cans/08-lifecycle.md#Lifecycle-state-machine
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShimConfig {
    pub max_exec_slots: u32,
    pub hide_pid: u32,
    pub anti_toctou_cgroups: bool,
    pub granular_task_freeze: bool,
    pub rejection_mode: RejectionMode,
    pub cgroup_parent: Option<String>,
}

impl Default for ShimConfig {
    fn default() -> Self {
        Self {
            max_exec_slots: 4096,
            hide_pid: 2,
            anti_toctou_cgroups: true,
            granular_task_freeze: true,
            rejection_mode: RejectionMode::SyntheticRefusal,
            cgroup_parent: None,
        }
    }
}

// see: cans/01-architecture.md#Boot-budget
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FastBootMode {
    #[default]
    Off,
    Auto,
    Enabled,
}

// see: cans/05-storage-plane.md#EROFS-rootfs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DaxWindowMode {
    #[default]
    Off,
    Auto,
    Fixed(u32),
}

// see: cans/15-operations.md#Developer-CLI
pub const HW_LOCKFILE_PATH: &str = "/etc/kelir/hw.lock";

// see: cans/15-operations.md#Developer-CLI
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HwLockRecord {
    pub probe: String,
    pub status: String,
    pub detail: String,
}

impl HwLockRecord {
    // see: cans/15-operations.md#Developer-CLI
    fn parse_line(line: &str) -> Option<Self> {
        let mut parts = line.splitn(3, ':');
        let probe = parts.next()?.trim();
        let status = parts.next()?.trim();
        let detail = parts.next()?.trim();
        if probe.is_empty() || status.is_empty() || detail.is_empty() {
            return None;
        }
        Some(Self {
            probe: probe.to_string(),
            status: status.to_string(),
            detail: detail.to_string(),
        })
    }

    // see: cans/15-operations.md#Developer-CLI
    pub fn passed(&self) -> bool {
        self.status == "pass"
    }
}

// see: cans/15-operations.md#Developer-CLI
pub fn parse_hw_lockfile(contents: &str) -> Result<Vec<HwLockRecord>, VmmError> {
    let mut records = Vec::new();
    for line in contents.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Some(record) = HwLockRecord::parse_line(line) else {
            return Err(VmmError::InvalidState(format!(
                "hardware lockfile carries an unattested line: {line}"
            )));
        };
        records.push(record);
    }
    if records.is_empty() {
        return Err(VmmError::InvalidState(
            "hardware lockfile attests no host probes".into(),
        ));
    }
    Ok(records)
}

// see: cans/15-operations.md#Developer-CLI
pub fn verify_hw_lockfile() -> Result<Vec<HwLockRecord>, VmmError> {
    let contents = std::fs::read_to_string(HW_LOCKFILE_PATH).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            VmmError::InvalidState(format!("hardware lockfile missing: {HW_LOCKFILE_PATH}"))
        } else {
            VmmError::Io(err)
        }
    })?;
    parse_hw_lockfile(&contents)
}

// see: cans/15-operations.md#Developer-CLI
fn required_hw_probes(backend: &str) -> Option<&'static [&'static str]> {
    match backend {
        "rustvmm" => Some(&["kvm"]),
        "hvf" => Some(&["hvf"]),
        "avf" => Some(&["avf"]),
        "openvmm" => {
            if cfg!(target_os = "windows") {
                Some(&["whp"])
            } else {
                Some(&["kvm"])
            }
        }
        _ => None,
    }
}

// see: cans/15-operations.md#Developer-CLI
fn live_hw_probe(probe: &str) -> Option<bool> {
    match probe {
        "kvm" => Some(Path::new("/dev/kvm").exists()),
        "avf" => Some(Path::new("/dev/avf").exists()),
        // TODO: [hypervisor_driver] fill in the L3 wave; live hvf/whp entitlement probes and page-size/pidfd verification need libc syscalls; the lockfile record is accepted until then.; slop: accepting unauthenticated lockfile as probed hardware
        _ => None,
    }
}

// see: cans/15-operations.md#Developer-CLI
pub fn attest_hw_lockfile(
    records: &[HwLockRecord],
    capabilities: &DriverCapabilities,
) -> Result<(), VmmError> {
    // TODO: [hypervisor_driver] fill in the L3 wave; verify the lockfile Ed25519 signature and the chattr +i immutable flag; the parsed records alone are unauthenticated ASCII.; slop: unsigned ASCII lockfile trusted as attested hardware
    let Some(required) = required_hw_probes(capabilities.backend) else {
        return Err(VmmError::InvalidState(format!(
            "hardware lockfile attestation has no probe mapping for backend `{}`",
            capabilities.backend
        )));
    };
    for probe in required {
        let Some(record) = records
            .iter()
            .find(|record| record.probe.as_str() == *probe)
        else {
            return Err(VmmError::InvalidState(format!(
                "hardware lockfile does not attest probe `{probe}` required by backend `{}`",
                capabilities.backend
            )));
        };
        if !record.passed() {
            return Err(VmmError::InvalidState(format!(
                "hardware lockfile probe `{probe}` attests `{}`: {}",
                record.status, record.detail
            )));
        }
        if live_hw_probe(probe) == Some(false) {
            return Err(VmmError::InvalidState(format!(
                "hardware lockfile probe `{probe}` attests pass but the host device is absent"
            )));
        }
    }
    Ok(())
}

// see: cans/15-operations.md#Developer-CLI
pub fn check_hw_lockfile(
    policy: &DriverPolicy,
    capabilities: &DriverCapabilities,
) -> Result<(), VmmError> {
    if !policy.verify_hw_lockfile || !capabilities.requires_hardware_vmm {
        return Ok(());
    }
    let records = verify_hw_lockfile()?;
    attest_hw_lockfile(&records, capabilities)
}

// see: cans/05-driver-plane.md#Deterministic-replay
pub fn reject_emulated_deterministic_clock(
    backend: &str,
    deterministic_clock: bool,
) -> Result<(), VmmError> {
    if !deterministic_clock {
        return Ok(());
    }
    Err(VmmError::Unsupported(format!(
        "{backend} deterministic_clock: a user-namespace sandbox cannot intercept VDSO clock reads; deterministic replay needs a hardware-vmm driver"
    )))
}

// see: cans/01-architecture.md#Embedded-profile
#[derive(Debug, Clone, PartialEq)]
pub struct VmConfig {
    pub vcpus: u8,
    pub memory_mb: u32,
    pub kernel_path: PathBuf,
    pub rootfs_erofs_path: PathBuf,
    // see: cans/03-sdk-architecture.md#Session-surfaces
    pub tenant_id: Option<String>,
    // TODO: [hypervisor_driver] fill in the L3 wave; consumed by hardware-vmm drivers selecting the guest MMU leaf mapping (4/16KB) at boot.; slop: hardcoded 4K mapping on 16K host, silent boot
    pub page_size_kb: u32,
    pub tsc_locked: bool,
    // TODO: [lc_23_replay_deterministic_seed_reproduces_step_windows] fill in the L3 wave; consumed by hardware-vmm replay drivers substituting a seeded ChaCha20 virtio-rng stream.; slop: unseeded RNG stream, single replay asserted complete
    pub entropy_seed: Option<u64>,
    pub driver_policy: DriverPolicy,
    pub enable_gpu_passthrough: bool,
    pub enable_watchdog: bool,
    pub deterministic_clock: bool,
    // TODO: [hypervisor_driver] fill in the L3 wave; consumed by hardware-vmm kernel boot-arg injection (clocksource=... tsc=reliable nohpet).; slop: invented boot args, no clocksource/tsc pinning
    pub pinned_clocksource: Option<String>,
    // TODO: [lc_23_replay_deterministic_seed_reproduces_step_windows] fill in the L3 wave; consumed by replay drivers stepping the guest clock one instruction at a time.; slop: free-running clock, single replay asserted complete
    pub ns_per_instruction: Option<u32>,
    // TODO: [lc_12_host_reaper_pdeathsig_enforcement] fill in the L3 wave; consumed by the kelir-engine host-crash reaper SIGTERM window (on_host_preempt hook).; slop: waitpid-only check, abrupt parent kill -9 untested
    pub preemption_grace_ms: u64,
    pub network_off: bool,
    // see: cans/artifacts/examples/fixtures/kelir.yaml#session.balloon
    pub balloon: Option<VmBalloonConfig>,
    // see: cans/artifacts/examples/fixtures/kelir.yaml#session.shim.mask_proc_environ
    pub mask_proc_environ: bool,
}

impl VmConfig {
    // see: cans/01-architecture.md#Embedded-profile
    pub fn missing_boot_path(&self) -> Option<&'static str> {
        if self.kernel_path.as_os_str().is_empty() {
            return Some("boot: kernel path is required (EINVAL)");
        }
        if self.rootfs_erofs_path.as_os_str().is_empty() {
            return Some("boot: rootfs erofs path is required (EINVAL)");
        }
        None
    }
}

// see: cans/01-architecture.md#Embedded-profile
impl Default for VmConfig {
    fn default() -> Self {
        Self {
            vcpus: 1,
            memory_mb: 256,
            kernel_path: PathBuf::new(),
            rootfs_erofs_path: PathBuf::new(),
            tenant_id: None,
            page_size_kb: 4,
            tsc_locked: true,
            entropy_seed: None,
            driver_policy: DriverPolicy::default(),
            enable_gpu_passthrough: false,
            enable_watchdog: false,
            deterministic_clock: true,
            pinned_clocksource: None,
            ns_per_instruction: None,
            preemption_grace_ms: 0,
            network_off: true,
            balloon: None,
            mask_proc_environ: false,
        }
    }
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#session.balloon
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VmBalloonConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub initial_mb: u32,
    #[serde(default)]
    pub pressure_reclaim: bool,
    // TODO: [hd_16_virtio_balloon_pressure_reclaim] fill in the L3 wave; madvise(MADV_DONTNEED) and hypervisor frame reclamation stay unimplemented behind the reclaim strategy string.; slop: reclaim accepted from size field alone, frames stay resident
    #[serde(default)]
    pub reclaim_strategy: String,
}

#[derive(Debug, Clone)]
pub struct GuestClock {
    pub absolute_wall_clock_ns: u64,
    pub monotonic_offset_ns: u64,
    pub timezone: String,
    pub boot_params: String,
}

// see: cans/09-snapshots.md#Archives-and-portability
#[derive(Debug, Clone)]
pub struct SnapshotDescriptor {
    pub id: String,
    pub dirty_bytes: u64,
    pub created_at_ns: u64,
    pub parent_digest: Option<String>,
    pub guest_clock: Option<GuestClock>,
}

#[derive(Debug, Clone)]
pub struct BalloonConvergence {
    pub target_mb: u32,
    pub converged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatchdogAction {
    Reboot,
    Terminate { emit_diagnostic: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatchdogVerdict {
    ExecuteDefault,
    Suppress,
    Mitigate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchdogEvent {
    pub timeout_ms: u64,
    pub action: WatchdogAction,
    pub fired_at_ns: u64,
    pub last_heartbeat_ms: u64,
    pub last_active_pid: Option<u32>,
}

// see: cans/08-lifecycle.md#Suspend-and-resume
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExternalSuspendKind {
    External(String),
    ExternalCode(u32),
}

// see: cans/artifacts/code/vmm-driver-trait.rs#SuspendReason
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuspendReason {
    External(ExternalSuspendKind),
    ResourcePressure,
}

#[derive(Debug, Clone)]
pub struct WatchdogDiagnostic {
    pub fired_at_ns: u64,
    pub last_heartbeat_ms: u64,
    pub last_active_pid: Option<u32>,
}

// see: cans/01-architecture.md#Shared-memory-channel
#[derive(Debug)]
pub struct SharedMemoryHandle {
    pub tag: String,
    pub size_bytes: usize,
    pub guest_phys_addr: u64,
    pub host_ptr: *mut u8,
    #[cfg(unix)]
    pub memfd_raw: std::os::unix::io::RawFd,
    #[cfg(windows)]
    pub section_handle: std::os::windows::io::RawHandle,
}

// see: cans/01-architecture.md#Shared-memory-channel
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShmMapping {
    pub tag: String,
    pub size_bytes: usize,
    pub guest_phys_addr: u64,
}

impl ShmMapping {
    // see: cans/01-architecture.md#Shared-memory-channel
    pub fn tag(&self) -> &str {
        &self.tag
    }

    // see: cans/01-architecture.md#Shared-memory-channel
    pub fn size_bytes(&self) -> usize {
        self.size_bytes
    }
}

// see: cans/01-architecture.md#Transport-engine-abstraction
pub trait Stream: Send + Sync {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error>;
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error>;
    fn flush(&mut self) -> Result<(), std::io::Error>;
}

// see: cans/05-driver-plane.md#Deterministic-replay
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntropyBackend {
    VirtioRng,
    SeededChaCha20 { seed: u64 },
}

// see: cans/05-driver-plane.md#Deterministic-replay
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntropyStreamPosition {
    pub offset_bytes: u64,
}

// see: cans/05-driver-plane.md#Deterministic-replay
#[derive(Debug, Clone, Copy)]
pub struct PmuSample {
    pub retired_instructions: u64,
    pub sampled_at_ns: u64,
}

// see: cans/05-driver-plane.md#Deterministic-replay
pub trait PmuSampler: Send + Sync {
    fn on_pmu_sample(&self, sample: &PmuSample);
}

// see: cans/artifacts/code/vmm-driver-trait.rs#VmmDriver
#[async_trait]
pub trait VmmDriver: Send + Sync {
    fn capabilities(&self) -> DriverCapabilities;
    async fn boot(&mut self, config: VmConfig) -> Result<(), VmmError>;
    // see: cans/01-architecture.md#Fallback-policy
    // TODO: [hd_09_bwrap_emulated_fallback_opt_in] fill in the L3 wave; backend drivers override is_emulated with the real emulation-tier probe.; slop: silent emulated fallback without explicit opt-in
    fn is_emulated(&self) -> Result<bool, VmmError> {
        Err(VmmError::Unsupported(
            "is_emulated: no native backend override (deferred to the L3 wave)".into(),
        ))
    }
    // TODO: [lc_01_cow_fork_warm_memory_sharing] fill in the L3 wave; backend drivers override fork with the native CoW clone primitive.; slop: memcpy clone instead of CoW page tables
    async fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError> {
        Err(VmmError::Unsupported(
            "fork: no native backend override (deferred to the L3 wave)".into(),
        ))
    }
    // TODO: [lc_base_snapshot_capture] fill in the L3 wave; backend drivers override checkpoint with atomic RAM + register capture.; slop: boot Ok asserted, capture ordering unchecked
    async fn checkpoint(&mut self, _label: &str) -> Result<SnapshotDescriptor, VmmError> {
        Err(VmmError::Unsupported(
            "checkpoint: no native backend override (deferred to the L3 wave)".into(),
        ))
    }
    // TODO: [hypervisor_driver] fill in the L3 wave; backend drivers override map_shared_region with the zero-copy guest segment map.; slop: copied guest segment instead of zero-copy map
    async fn map_shared_region(
        &mut self,
        _tag: &str,
        _size_bytes: usize,
    ) -> Result<SharedMemoryHandle, VmmError> {
        Err(VmmError::Unsupported(
            "map_shared_region: no native backend override (deferred to the L3 wave)".into(),
        ))
    }
    // TODO: [lc_20_portable_snapshot_archive_export] fill in the L3 wave; backend drivers override export_snapshot with the portable snapshot stream.; slop: export Ok, trailer digest never verified
    async fn export_snapshot(&self) -> Result<Box<dyn AsyncRead + Send + Sync + Unpin>, VmmError> {
        Err(VmmError::Unsupported(
            "export_snapshot: no native backend override (deferred to the L3 wave)".into(),
        ))
    }
    // TODO: [lc_20_portable_snapshot_archive_export] fill in the L3 wave; backend drivers override import_snapshot with the portable snapshot hydrate.; slop: hydrate Ok, manifest digest never verified
    async fn import_snapshot(
        &mut self,
        _reader: Box<dyn AsyncRead + Send + Sync + Unpin>,
    ) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "import_snapshot: no native backend override (deferred to the L3 wave)".into(),
        ))
    }
    // TODO: [hd_03_hardware_rollback_timing_bound] fill in the L3 wave; backend drivers override rollback with register reset + memory revert in <8ms.; slop: rollback Ok asserted, 8ms bound never measured
    async fn rollback(&mut self, _snapshot: &SnapshotDescriptor) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "rollback: no native backend override (deferred to the L3 wave)".into(),
        ))
    }
    // TODO: [lc_02_suspend_zero_rss_purge] fill in the L3 wave; backend drivers override suspend with dirty-page flush to disk in <50ms.; slop: vCPU paused, RAM pages never unmapped
    async fn suspend(
        &mut self,
        _reason: SuspendReason,
        _output_path: &Path,
    ) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "suspend: no native backend override (deferred to the L3 wave)".into(),
        ))
    }
    // TODO: [lifecycle_checkpoint] fill in the L3 wave; backend drivers override resume with suspended-image restore in <50ms.; slop: resume Ok asserted, 50ms restore bound never measured
    async fn resume(&mut self, _image_path: &Path) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "resume: no native backend override (deferred to the L3 wave)".into(),
        ))
    }
    // TODO: [lc_11_lid_sleep_clock_resync_snap] fill in the L3 wave; backend drivers override skew_clock with the KVM_SET_CLOCK / CNTVOFF_EL2 offset.; slop: offset applied, guest clock jump never verified
    async fn skew_clock(&mut self, _offset: Duration) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "skew_clock: no native backend override (deferred to the L3 wave)".into(),
        ))
    }
    // TODO: [hd_16_virtio_balloon_pressure_reclaim] fill in the L3 wave; backend drivers override adjust_balloon with the virtio-balloon resize.; slop: balloon Ok, host RSS reduction never checked
    async fn adjust_balloon(&mut self, _target_mb: u32) -> Result<BalloonConvergence, VmmError> {
        Err(VmmError::Unsupported(
            "adjust_balloon: no native backend override (deferred to the L3 wave)".into(),
        ))
    }
    // see: cans/10-attestation.md#Lifecycle-bridge
    // TODO: [hypervisor_driver] fill in the L3 wave; backend drivers override freeze_vcpus with the cycle-boundary vCPU halt.; slop: halt Ok, cycle-boundary stop never verified
    async fn freeze_vcpus(&mut self) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "freeze_vcpus: no native backend override (deferred to the L3 wave)".into(),
        ))
    }
    // see: cans/11-protocol.md#Binary-exec-channel
    // TODO: [hypervisor_driver] fill in the L3 wave; backend drivers override freeze_task with the trapped-task cgroup freeze; whole-VM freeze_vcpus stays the fallback.; slop: task freeze unimplemented, whole VM frozen instead
    async fn freeze_task(&mut self, _task_cgroup: &str) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "freeze_task: no native backend override (deferred to the L3 wave)".into(),
        ))
    }
    // see: cans/10-attestation.md#Lifecycle-bridge
    // TODO: [hypervisor_driver] fill in the L3 wave; backend drivers override unfreeze_vcpus with the vCPU scheduler release.; slop: release Ok, vCPU scheduler state never verified
    async fn unfreeze_vcpus(&mut self) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "unfreeze_vcpus: no native backend override (deferred to the L3 wave)".into(),
        ))
    }
    // see: cans/05-driver-plane.md#Two-phase-commit
    // TODO: [rt_03_2pc_pre_commit_vcpu_pause_drain] fill in the L3 wave; backend drivers override drain_fuse with the in-flight FUSE drain.; slop: commit issued while vCPUs still executing
    async fn drain_fuse(&mut self, _timeout: Duration) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "drain_fuse: no native backend override (deferred to the L3 wave)".into(),
        ))
    }
    // see: cans/05-driver-plane.md#Two-phase-commit
    // TODO: [relational_transaction] fill in the L3 wave; backend drivers override commit_broker_upper with the broker upper commit.; slop: commit Ok, broker upper never replayed to disk
    async fn commit_broker_upper(&mut self) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "commit_broker_upper: no native backend override (deferred to the L3 wave)".into(),
        ))
    }
    // see: cans/05-driver-plane.md#Two-phase-commit
    // TODO: [relational_transaction] fill in the L3 wave; backend drivers override discard_broker_upper with the broker upper discard.; slop: discard Ok, broker upper file never deleted
    async fn discard_broker_upper(&mut self) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "discard_broker_upper: no native backend override (deferred to the L3 wave)".into(),
        ))
    }
    // see: cans/05-driver-plane.md#Deterministic-replay
    // TODO: [lc_23_replay_deterministic_seed_reproduces_step_windows] fill in the L3 wave; backend drivers override step_clock_window with the retired-instruction clock step.; slop: single replay asserted, no window-for-window comparison
    async fn step_clock_window(&mut self, _retired_instructions: u64) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "step_clock_window: no native backend override (deferred to the L3 wave)".into(),
        ))
    }
    // see: cans/05-driver-plane.md#Deterministic-replay
    // TODO: [hypervisor_driver] fill in the L3 wave; backend drivers override register_pmu_sampler with the host PMU counter arming.; slop: sampler Ok, host PMU counters never armed
    fn register_pmu_sampler(&mut self, _sampler: Arc<dyn PmuSampler>) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "register_pmu_sampler: no native backend override (deferred to the L3 wave)".into(),
        ))
    }
    // see: cans/05-driver-plane.md#Deterministic-replay
    async fn set_entropy_backend(&mut self, backend: EntropyBackend) -> Result<(), VmmError>;
    // see: cans/05-driver-plane.md#Deterministic-replay
    async fn entropy_position(&self) -> Result<EntropyStreamPosition, VmmError>;
    // see: cans/05-driver-plane.md#Deterministic-replay
    async fn restore_entropy_position(
        &mut self,
        position: EntropyStreamPosition,
    ) -> Result<(), VmmError>;
    // TODO: [lc_18_watchdog_heartbeat_timeout_reboot] fill in the L3 wave; backend drivers override set_watchdog with the hardware watchdog arm.; slop: reboot asserted, vCPU registers never reset to 0
    async fn set_watchdog(
        &mut self,
        _timeout: Duration,
        _action: WatchdogAction,
    ) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "set_watchdog: no native backend override (deferred to the L3 wave)".into(),
        ))
    }
    // TODO: [hypervisor_driver] fill in the L3 wave; backend drivers override transport_connect with the raw duplex channel.; slop: channel Ok, duplex bytes never cross raw transport
    fn transport_connect(&self, _channel_id: u32) -> Result<Box<dyn Stream>, VmmError> {
        Err(VmmError::Unsupported(
            "transport_connect: no native backend override (deferred to the L3 wave)".into(),
        ))
    }
    // TODO: [hypervisor_driver] fill in the L3 wave; backend drivers override terminate with the hypervisor teardown.; slop: teardown Ok, guest resources never released
    async fn terminate(&mut self) -> Result<(), VmmError> {
        Err(VmmError::Unsupported(
            "terminate: no native backend override (deferred to the L3 wave)".into(),
        ))
    }
}
