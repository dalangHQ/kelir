// crates/kelir-interface/src/driver.rs
//
// The universal hypervisor backend abstraction.
//
// Owned by: cans/01-architecture.md#VmmDriver-trait

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncRead;

/// Granular memory execution policy enforced by the hypervisor and guest trap loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WxPolicy {
    /// Hard panic on anonymous executable mappings or dynamic mprotect(PROT_EXEC).
    Strict,
    /// Log violation to host ledger and continue execution.
    PermissiveAudit,
}

/// Host virtualization selection and hardware validation parameters.
#[derive(Debug, Clone)]
pub struct DriverPolicy {
    /// Refuse to initialize if true hardware virtualization (KVM/HVF/AVF) is missing.
    pub require_hardware_vmm: bool,
    /// Explicit flag required to permit bwrap or proot fallback execution.
    pub allow_emulated_fallback: bool,
    /// Verify host integrity lockfile (/etc/kelir/hw.lock) before allocating vCPUs.
    pub verify_hw_lockfile: bool,
    /// Memory execution boundary policy.
    pub wx_enforcement: WxPolicy,
    /// Size of dedicated worker thread pool servicing foreign runtime FFI events.
    pub ffi_worker_threads: usize,
    /// Maximum event dispatch operations per second permitted across FFI boundary.
    pub ffi_max_ops_per_sec: u32,
}

#[derive(Debug, Clone)]
pub struct VmConfig {
    /// Elastic roster: session.vm.set_vcpus(n) parks/unparks host vCPU
    /// threads and onlines/offlines guest cores with no reboot, no
    /// teardown, and no session re-attestation.
    pub vcpus: u8,
    pub memory_mb: u32,
    pub kernel_path: PathBuf,
    pub rootfs_erofs_path: PathBuf,
    /// Guest page size in KB (4 or 16)
    /// - The host reads its own page size and selects the matching vmlinux
    ///   fixture before cold boot; see cans/01-architecture.md#Guest-VM-anatomy
    pub page_size_kb: u32,
    pub tsc_locked: bool,
    /// Replay-mode seed: when replay is active, the virtio-rng backend
    /// is substituted with a seeded ChaCha20 stream and this seed is
    /// captured in the snapshot manifest, so getrandom() payloads match
    /// byte-for-byte across runs. Outside replay, virtio-rng is the
    /// mandatory, unconfigurable host-entropy device (Linux
    /// /dev/urandom, macOS getentropy(2)); a profile cannot remove it.
    pub entropy_seed: Option<u64>,
    pub driver_policy: DriverPolicy,
    pub enable_gpu_passthrough: bool,
    pub enable_watchdog: bool,
    pub deterministic_clock: bool,
    pub pinned_clocksource: Option<String>,
    pub ns_per_instruction: Option<u32>,
    pub preemption_grace_ms: u64,
}

/// Guest clock captured with a snapshot, stored as guest_clock.json in
/// the snapshot manifest. Restores set both clocks from it before
/// unfreezing, so guest time leaps from capture to restore.
#[derive(Debug, Clone)]
pub struct GuestClock {
    pub absolute_wall_clock_ns: u64,
    pub monotonic_offset_ns: u64,
    pub timezone: String, // IANA name, or "UTC"
    pub boot_params: String, // e.g. clock source, nohpet
}

#[derive(Debug, Clone)]
pub struct SnapshotDescriptor {
    pub id: String,
    /// Byte count; see cans/01-architecture.md#VmmDriver-trait
    pub dirty_bytes: u64,
    pub created_at_ns: u64,
    /// Parent in the snapshot chain. Depth is unbounded and restore
    /// accepts any digest in the chain, not only the leaf; the restored
    /// session's working delta becomes a new child of that digest.
    /// child_digests live on the catalog row, not on this descriptor.
    pub parent_digest: Option<String>,
    pub guest_clock: Option<GuestClock>,
}

#[derive(Debug, Clone)]
pub enum WatchdogAction {
    /// Guest kernel lockup detected → hard-reset the vCPUs and reboot the microVM
    Reboot,
    /// Unresponsive VM → emit structured diagnostic payload, then terminate instance
    Terminate { emit_diagnostic: bool },
}

/// Generalized suspend reason. Carries no plugin identity:
/// an external trigger is identified only by an opaque code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalSuspendKind {
    HumanAuthorizationHold = 0x0001,
    WireConnectionParking  = 0x0002,
    StepEvaluationYield    = 0x0003,
    HostPowerEventSnapshot = 0x0004,
}

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

#[async_trait]
pub trait VmmDriver: Send + Sync {
    /// Cold-boots the microVM environment in <60ms via the active platform driver
    /// - The host page size selects the 4KB or 16KB kernel fixture
    /// - The embedded profile boots 1 vCPU with 256MB RAM
    async fn boot(&mut self, config: VmConfig) -> Result<(), VmmError>;

    /// Branches the running VM into an isolated CoW clone in <15ms
    /// - Linux: userfaultfd + KVM memory cloning
    /// - macOS: HVF memory-mapped page duplication
    async fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError>;

    /// Creates an atomic in-memory checkpoint of guest RAM and vCPU registers
    async fn checkpoint(&mut self, label: &str) -> Result<SnapshotDescriptor, VmmError>;

    /// Maps an anonymous zero-copy host memory segment into guest address space
    async fn map_shared_region(&mut self, tag: &str, size_bytes: usize) -> Result<SharedMemoryHandle, VmmError>;

    /// Serializes registers and compressed dirty pages into a portable stream
    async fn export_snapshot(&self) -> Result<Box<dyn AsyncRead + Send + Sync + Unpin>, VmmError>;

    /// Hydrates microVM registers and memory pages from a portable snapshot stream
    async fn import_snapshot(&mut self, reader: Box<dyn AsyncRead + Send + Sync + Unpin>) -> Result<(), VmmError>;

    /// Reverts guest physical RAM and vCPU registers to a prior checkpoint in <8ms.
    /// Host filesystem state is the sparse upperdir delta, discarded by
    /// kelir-core Session::rollback (truncate / punch holes), not a tmpfs diff.
    async fn rollback(&mut self, snapshot: &SnapshotDescriptor) -> Result<(), VmmError>;

    /// Flushes dirty RAM pages to disk and drops host memory consumption to 0 MB.
    /// Reason is SuspendReason::External or ResourcePressure; no plugin identity.
    async fn suspend(&mut self, reason: SuspendReason, output_path: &Path) -> Result<(), VmmError>;

    /// Restores a suspended VM image from storage in <50ms.
    /// Host samples CLOCK_REALTIME and dispatches a paravirtual clock
    /// update frame over Channel 0x0000; kelir-shim issues
    /// clock_settime(CLOCK_REALTIME) before unfreezing cgroups or
    /// releasing trapped tasks, so guest time snaps forward instantly.
    async fn resume(&mut self, image_path: &Path) -> Result<(), VmmError>;

    /// Offsets the virtual hardware clock (KVM_SET_CLOCK / CNTVOFF_EL2)
    async fn skew_clock(&mut self, offset: Duration) -> Result<(), VmmError>;

    /// Adjusts guest RAM ceiling dynamically via virtio-balloon
    async fn adjust_balloon(&mut self, target_mb: u32) -> Result<(), VmmError>;

    /// Arms the hardware watchdog: when no heartbeat arrives within `timeout`,
    /// the driver executes `action` (Reboot / Terminate)
    async fn set_watchdog(&mut self, timeout: Duration, action: WatchdogAction) -> Result<(), VmmError>;

    /// Yields raw duplex channel to the Transport backend multiplexer
    /// (SPSC rings primary; AF_VSOCK / named-pipe fallback). The port
    /// argument is the logical channel identifier (5252 is a channel
    /// identifier, not an OS socket number). Method name kept for
    /// compatibility; it does not imply AF_VSOCK semantics.
    fn vsock_connect(&self, port: u32) -> Result<tokio::net::UnixStream, VmmError>;

    /// Terminates hypervisor instance and releases memory allocations
    async fn terminate(&mut self) -> Result<(), VmmError>;
}
