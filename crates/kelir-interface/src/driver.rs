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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverState {
    Unbooted,
    Booted,
    Paused,
}

impl DriverState {
    // TODO (L3): drivers stay Unbooted until the boot handshake lands, so every guarded primitive rejects here; drop the (L3) suffix once boot is real per cans/08-lifecycle.md#Hypervisor-primitives.
    pub fn ensure_booted(&self, backend: &str) -> Result<(), VmmError> {
        match self {
            Self::Booted | Self::Paused => Ok(()),
            Self::Unbooted => Err(VmmError::InvalidState(format!(
                "{backend} driver is not booted (L3)"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WxPolicy {
    Strict,
    PermissiveAudit,
}

#[derive(Debug, Clone)]
pub struct DriverPolicy {
    pub require_hardware_vmm: bool,
    pub allow_emulated_fallback: bool,
    pub verify_hw_lockfile: bool,
    pub wx_enforcement: WxPolicy,
    pub ffi_worker_threads: usize,
    pub ffi_max_ops_per_sec: u32,
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
        // TODO (L3): live hvf/whp entitlement probes and page-size/pidfd verification need libc syscalls; the lockfile record is accepted until then.
        _ => None,
    }
}

// see: cans/15-operations.md#Developer-CLI
pub fn attest_hw_lockfile(
    records: &[HwLockRecord],
    capabilities: &DriverCapabilities,
) -> Result<(), VmmError> {
    // TODO (L3): verify the lockfile Ed25519 signature and the chattr +i immutable flag; the parsed records alone are unauthenticated ASCII.
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

// see: cans/plugins/replay.md#Invariants-and-policy-(replay)
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

#[derive(Debug, Clone)]
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
pub fn default_vm_config(
    vcpus: Option<u8>,
    memory_mb: Option<u32>,
    kernel: Option<String>,
    rootfs_erofs: Option<String>,
    tsc_locked: Option<bool>,
    enable_watchdog: Option<bool>,
) -> VmConfig {
    VmConfig {
        vcpus: vcpus.unwrap_or(1),
        memory_mb: memory_mb.unwrap_or(256),
        kernel_path: PathBuf::from(kernel.unwrap_or_default()),
        rootfs_erofs_path: PathBuf::from(rootfs_erofs.unwrap_or_default()),
        page_size_kb: 4,
        tsc_locked: tsc_locked.unwrap_or(true),
        entropy_seed: None,
        driver_policy: DriverPolicy {
            require_hardware_vmm: true,
            allow_emulated_fallback: false,
            verify_hw_lockfile: false,
            wx_enforcement: WxPolicy::Strict,
            // see: cans/01-architecture.md#Foreign-runtime-embedding
            ffi_worker_threads: 1,
            ffi_max_ops_per_sec: 0,
        },
        enable_gpu_passthrough: false,
        enable_watchdog: enable_watchdog.unwrap_or(false),
        deterministic_clock: true,
        pinned_clocksource: None,
        ns_per_instruction: None,
        preemption_grace_ms: 0,
        network_off: true,
    }
}

#[derive(Debug, Clone)]
pub struct GuestClock {
    pub absolute_wall_clock_ns: u64,
    pub monotonic_offset_ns: u64,
    pub timezone: String,
    pub boot_params: String,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

// see: cans/artifacts/code/vmm-driver-trait.rs#ExternalSuspendKind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalSuspendKind {
    HumanAuthorizationHold = 0x0001,
    WireConnectionParking = 0x0002,
    StepEvaluationYield = 0x0003,
    HostPowerEventSnapshot = 0x0004,
}

// see: cans/artifacts/code/vmm-driver-trait.rs#SuspendReason
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

// see: cans/01-architecture.md#Transport-engine-abstraction
pub trait Stream: Send + Sync {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error>;
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error>;
    fn flush(&mut self) -> Result<(), std::io::Error>;
}

// see: cans/plugins/replay.md#Deterministic-RNG
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntropyBackend {
    VirtioRng,
    SeededChaCha20 { seed: u64 },
}

// see: cans/plugins/replay.md#Deterministic-RNG
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntropyStreamPosition {
    pub offset_bytes: u64,
}

// see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
#[derive(Debug, Clone, Copy)]
pub struct PmuSample {
    pub retired_instructions: u64,
    pub sampled_at_ns: u64,
}

// see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
pub trait PmuSampler: Send + Sync {
    fn on_pmu_sample(&self, sample: &PmuSample);
}

// see: cans/artifacts/code/vmm-driver-trait.rs#VmmDriver
#[async_trait]
pub trait VmmDriver: Send + Sync {
    fn capabilities(&self) -> DriverCapabilities;
    async fn boot(&mut self, config: VmConfig) -> Result<(), VmmError>;
    async fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError>;
    async fn checkpoint(&mut self, label: &str) -> Result<SnapshotDescriptor, VmmError>;
    async fn map_shared_region(
        &mut self,
        tag: &str,
        size_bytes: usize,
    ) -> Result<SharedMemoryHandle, VmmError>;
    async fn export_snapshot(&self) -> Result<Box<dyn AsyncRead + Send + Sync + Unpin>, VmmError>;
    async fn import_snapshot(
        &mut self,
        reader: Box<dyn AsyncRead + Send + Sync + Unpin>,
    ) -> Result<(), VmmError>;
    async fn rollback(&mut self, snapshot: &SnapshotDescriptor) -> Result<(), VmmError>;
    async fn suspend(&mut self, reason: SuspendReason, output_path: &Path) -> Result<(), VmmError>;
    async fn resume(&mut self, image_path: &Path) -> Result<(), VmmError>;
    async fn skew_clock(&mut self, offset: Duration) -> Result<(), VmmError>;
    async fn adjust_balloon(&mut self, target_mb: u32) -> Result<BalloonConvergence, VmmError>;
    // see: cans/10-attestation.md#Lifecycle-bridge
    async fn freeze_vcpus(&mut self) -> Result<(), VmmError>;
    async fn unfreeze_vcpus(&mut self) -> Result<(), VmmError>;
    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    async fn drain_fuse(&mut self, timeout: Duration) -> Result<(), VmmError>;
    async fn commit_broker_upper(&mut self) -> Result<(), VmmError>;
    async fn discard_broker_upper(&mut self) -> Result<(), VmmError>;
    // see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
    async fn step_clock_window(&mut self, retired_instructions: u64) -> Result<(), VmmError>;
    fn register_pmu_sampler(&mut self, sampler: Arc<dyn PmuSampler>) -> Result<(), VmmError>;
    // see: cans/plugins/replay.md#Deterministic-RNG
    async fn set_entropy_backend(&mut self, backend: EntropyBackend) -> Result<(), VmmError>;
    async fn entropy_position(&self) -> Result<EntropyStreamPosition, VmmError>;
    async fn restore_entropy_position(
        &mut self,
        position: EntropyStreamPosition,
    ) -> Result<(), VmmError>;
    async fn set_watchdog(
        &mut self,
        timeout: Duration,
        action: WatchdogAction,
    ) -> Result<(), VmmError>;
    fn transport_connect(&self, channel_id: u32) -> Result<Box<dyn Stream>, VmmError>;
    async fn terminate(&mut self) -> Result<(), VmmError>;
}
