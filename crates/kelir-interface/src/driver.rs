// see: cans/01-architecture.md#VmmDriver-trait
// see: cans/08-lifecycle.md#Hypervisor-primitives
use std::path::PathBuf;

pub use crate::errors::{DriverError, VmmError};

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

#[derive(Debug, Clone)]
pub enum WatchdogAction {
    Reboot,
    Terminate { emit_diagnostic: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchdogVerdict {
    ExecuteDefault,
    Suppress,
    Mitigate,
}

#[derive(Debug, Clone)]
pub struct WatchdogEvent {
    pub timeout_ms: u64,
    pub action: WatchdogAction,
    pub fired_at_ns: u64,
    pub last_heartbeat_ms: u64,
    pub last_active_pid: Option<u32>,
}

// see: cans/artifacts/code/vmm-driver-trait.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalSuspendKind {
    HumanAuthorizationHold = 0x0001,
    WireConnectionParking = 0x0002,
    StepEvaluationYield = 0x0003,
    HostPowerEventSnapshot = 0x0004,
}

// see: cans/artifacts/code/vmm-driver-trait.rs
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

#[derive(Debug)]
pub struct SharedMemoryHandle {
    pub tag: String,
    pub size_bytes: usize,
    pub guest_phys_addr: u64,
    pub host_ptr: *mut u8,
    pub memfd_raw: std::os::unix::io::RawFd,
}

// see: cans/artifacts/code/vmm-driver-trait.rs
pub trait VmmDriver: Send + Sync {
    fn boot(&mut self, config: &VmConfig) -> Result<(), DriverError>;
    fn pause(&mut self) -> Result<(), DriverError>;
    fn resume(&mut self) -> Result<(), DriverError>;
    fn step(&mut self) -> Result<(), DriverError>;
    fn fork(&self) -> Result<Box<dyn VmmDriver>, DriverError>;
    fn rollback(&mut self, snapshot: &SnapshotDescriptor) -> Result<(), DriverError>;
    fn snapshot(&mut self) -> Result<SnapshotDescriptor, DriverError>;
    fn set_vcpus(&mut self, count: u8) -> Result<(), DriverError>;
    fn adjust_balloon(&mut self, target_mb: u32) -> Result<(), DriverError>;
    fn skew_clock(&mut self, offset_ns: i64) -> Result<(), DriverError>;
}
