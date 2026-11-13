// crates/kelir-interface/src/driver_interface.flow.rs
//
// The universal hypervisor backend abstraction.
// Interface enforces #![forbid(unsafe_code)].
// Pluggable backend drivers (driver-rustvmm, driver-hvf, driver-openvmm)
// isolate required ioctls and raw pointer operations under #![deny(unsafe_code)],
// with mandatory // SAFETY: invariants documented on every unsafe block.
//
// Owned by: cans/01-architecture.md#VmmDriver-trait

#![forbid(unsafe_code)]

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncRead;

// normative mirror of crates/kelir-interface/src/driver_interface.flow.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FastBootMode {
    #[default]
    Off,
    Auto,
    Enabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DaxWindowMode {
    #[default]
    Off,
    Auto,
    Fixed(u32),
}

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
    /// Backend driver selection override; None leaves the choice to the backend picker.
    /// see: cans/01-architecture.md#Driver-backends
    pub driver: Option<String>,
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
    /// Replay profiles declare network: off (plugins/replay.md): when
    /// true, the wire plane drops all egress for this session.
    pub network_off: bool,
}

/// Guest clock captured with a snapshot, stored as guest_clock.json in
/// the snapshot manifest. Restores set both clocks from it before
/// unfreezing, so guest time leaps from capture to restore.
/// Restored monotonic time clamps to max(current_monotonic,
/// snapshot_monotonic): CLOCK_MONOTONIC never regresses across restore,
/// and fork branches never produce negative deltas.
#[derive(Debug, Clone)]
pub struct GuestClock {
    pub absolute_wall_clock_ns: u64,
    pub monotonic_offset_ns: u64,
    pub timezone: String, // IANA name, or "UTC"
    pub boot_params: String, // e.g. clock source, nohpet
}

///
/// Compile-time tripwire: Any field added to SnapshotDescriptor mandates
/// immediate explicit serialization mapping in kelir-engine and kelir-catalog;
/// silent field omission fails the workspace exhaustiveness assert.
#[derive(Debug, Clone)]
pub struct SnapshotDescriptor {
    pub id: String,
    /// Byte count; see cans/01-architecture.md#VmmDriver-trait
    pub dirty_bytes: u64,
    pub created_at_ns: u64,
    /// Parent in the snapshot chain. Depth is unbounded and restore
    /// accepts any digest in the chain, not only the leaf; the restored
    /// session's working delta becomes a new child of that digest.
    /// The catalog stores zero child arrays: each snapshot manifest
    /// immutably records its parent_digest, and lineage is resolved by
    /// CAS manifest graph walks. Separately, every session registers a
    /// catalog row, no bypass: ephemeral lease_kind='ephemeral',
    /// expires_at NULL, reaped on parent death; detached
    /// lease_kind='lease', expires_at refreshed 30s; the row carries
    /// incarnation_id (UUIDv7 per spawn), boot_id, start_time_ns, pid;
    /// liveness proof Linux pidfd, macOS kqueue NOTE_EXIT, Windows Job
    /// Object KILL_ON_JOB_CLOSE.
    pub parent_digest: Option<String>,
    pub guest_clock: Option<GuestClock>,
}

/// Catalog session row (06-storage-catalog / 08-lifecycle). Every
/// session registers a row, no bypass.
/// incarnation_id is a UUIDv7 minted per spawn, in string form.
/// expires_at_unix is NULL (None) for ephemeral rows and set for
/// detached rows (refreshed every 30s). Ephemeral rows are reaped on
/// parent-process death, detached rows on lease TTL expiry, and the
/// janitor reaps both kinds on runtime child startup. Liveness proof:
/// Linux pidfd, macOS kqueue NOTE_EXIT, Windows Job Object
/// KILL_ON_JOB_CLOSE. Any (boot_id, start_time_ns, pid) mismatch
/// marks the row Crashed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseKind {
    Ephemeral,
    Lease,
}

#[derive(Debug, Clone)]
pub struct SessionRow {
    pub session_id: String,
    pub status: String,
    pub incarnation_id: String,
    pub boot_id: String,
    pub start_time_ns: u64,
    pub pid: u32,
    pub lease_kind: LeaseKind,
    pub expires_at_unix: Option<u64>,
}

/// Handle returned by adjust_balloon. converged becomes true on guest
/// ack (VIRTIO_BALLOON_S_MEMFREE / S_AVAIL); host reclaim gates on
/// guest ack; fork, snapshot, and heavy exec require prior convergence
/// completion.
#[derive(Debug, Clone)]
pub struct BalloonConvergence {
    pub target_mb: u32,
    pub converged: bool,
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

/// Driver-side error for every VmmDriver method. Distinct from
/// PluginError (plugin-side, plugin-trait.rs): driver failures are
/// backend failures, reported here so drivers never borrow the
/// plugin error type. Variants cover the backend's own failure modes;
/// a driver that only has an OS error reports it via Io.
#[derive(Debug)]
pub enum VmmError {
    /// Backend does not support the requested operation on this host.
    Unsupported(String),
    /// Backend configuration or state is invalid for the operation.
    InvalidState(String),
    /// Underlying OS / hypervisor error.
    Io(std::io::Error),
    /// Guest or host resource exhausted (memory, fds, vCPUs).
    ResourceExhausted(String),
}

impl std::fmt::Display for VmmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VmmError::Unsupported(m) => write!(f, "unsupported: {}", m),
            VmmError::InvalidState(m) => write!(f, "invalid state: {}", m),
            VmmError::Io(e) => write!(f, "io: {}", e),
            VmmError::ResourceExhausted(m) => write!(f, "exhausted: {}", m),
        }
    }
}

impl std::error::Error for VmmError {}

impl From<std::io::Error> for VmmError {
    fn from(e: std::io::Error) -> Self {
        VmmError::Io(e)
    }
}

#[async_trait]
pub trait VmmDriver: Send + Sync {
    /// Cold-boots the microVM environment in <60ms via the active platform driver
    /// - The host page size selects the 4KB or 16KB kernel fixture
    /// - The embedded profile boots 1 vCPU with 256MB RAM
    async fn boot(&mut self, config: VmConfig) -> Result<(), VmmError>;

    /// Branches the running VM into an isolated CoW clone in <15ms
    /// - Linux: userfaultfd + KVM memory cloning
    /// - macOS: HVF Stage-2 chunked page promoter: Stage-2 data aborts
    ///   promote eager 2MB blocks via Mach vm_copy CoW; children inherit
    ///   zero CoreFoundation state, SCDynamicStore/Keychain stay with the
    ///   pre-fork supervisor, and forked children resolve DNS via POSIX
    ///   structures (no SIGABRT); no vCPU exit storms on large heaps
    async fn fork(&self) -> Result<Box<dyn VmmDriver>, VmmError>;

    /// Creates an atomic in-memory checkpoint of guest RAM and vCPU registers
    /// Pre-checkpoint dentry trim: VirtIO-FS issues an active FUSE forget
    /// pass; unreferenced dentries with zero open handles are evicted
    /// before capture, preventing descriptor explosion from recursive sweeps
    async fn checkpoint(&mut self, label: &str) -> Result<SnapshotDescriptor, VmmError>;

    /// Maps an anonymous zero-copy host memory segment into guest address space
    async fn map_shared_region(&mut self, tag: &str, size_bytes: usize) -> Result<SharedMemoryHandle, VmmError>;

    /// Serializes registers and compressed dirty pages into a portable stream
    async fn export_snapshot(&self) -> Result<Box<dyn AsyncRead + Send + Sync + Unpin>, VmmError>;

    /// Hydrates microVM registers and memory pages from a portable snapshot stream
    async fn import_snapshot(&mut self, reader: Box<dyn AsyncRead + Send + Sync + Unpin>) -> Result<(), VmmError>;

    /// Reverts guest physical RAM and vCPU registers to a prior checkpoint in <8ms.
    /// Host filesystem state is the host CoW broker per-session sparse
    /// upper file + explicit tombstones (VirtIO-FS only; no in-guest
    /// OverlayFS, no virtio-blk workspace disk): rollback drops the
    /// upper file and tombstones, base untouched, FALLOC_FL_PUNCH_HOLE
    /// + posix_fadvise(DONTNEED) on the upper. Rollback also closes
    /// host sockets; the guest sees ECONNRESET/EPIPE, and sockets are
    /// not restored from snapshots.
    async fn rollback(&mut self, snapshot: &SnapshotDescriptor) -> Result<(), VmmError>;

    /// Flushes dirty RAM pages to disk and drops host memory consumption to 0 MB in <50ms.
    /// Reason is SuspendReason::External or ResourcePressure; no plugin identity.
    async fn suspend(&mut self, reason: SuspendReason, output_path: &Path) -> Result<(), VmmError>;

    /// Restores a suspended VM image from storage in <50ms.
    /// Host samples CLOCK_REALTIME and dispatches a paravirtual clock
    /// update frame over Control transport 0x0000; kelir-shim issues
    /// clock_settime(CLOCK_REALTIME) before unfreezing cgroups or
    /// releasing trapped tasks, so guest time snaps forward instantly.
    async fn resume(&mut self, image_path: &Path) -> Result<(), VmmError>;

    /// Offsets the virtual hardware clock (KVM_SET_CLOCK / CNTVOFF_EL2)
    async fn skew_clock(&mut self, offset: Duration) -> Result<(), VmmError>;

    /// Adjusts guest RAM ceiling dynamically via virtio-balloon.
    /// Returns a BalloonConvergence handle, not void. Host subscribes
    /// to VIRTIO_BALLOON_S_MEMFREE / S_AVAIL; host reclaim gates on
    /// guest ack; fork, snapshot, and heavy exec require prior
    /// convergence.
    async fn adjust_balloon(&mut self, target_mb: u32) -> Result<BalloonConvergence, VmmError>;

    /// Arms the hardware watchdog: when no heartbeat arrives within `timeout`,
    /// the driver executes `action` (Reboot / Terminate)
    async fn set_watchdog(&mut self, timeout: Duration, action: WatchdogAction) -> Result<(), VmmError>;

    /// Yields raw duplex channel to the Transport backend multiplexer.
    fn transport_connect(&self, channel_id: u32) -> Result<tokio::net::UnixStream, VmmError>;

    /// Terminates hypervisor instance and releases memory allocations
    async fn terminate(&mut self) -> Result<(), VmmError>;
}
