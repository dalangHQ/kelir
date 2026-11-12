// crates/kelir-interface/src/protocol_wire.rs
//
// All wire messages exchanged over transport port 5252.
// 5252 is a transport identifier, not an AF_VSOCK port: the Transport
// backend carries it (shared-memory SPSC rings primary; AF_VSOCK on
// Linux/macOS and named pipes on Windows as debug fallbacks).
// Exactly two transports: Control 0x0000 and Stream Mux 0x0001;
// every other capability is a stream kind multiplexed on 0x0001
// (see wire-protocol.rs for the kind registry).
// Payloads encode as CBOR inside the TLV frame defined in codec_protocol.rs.
//
// Owned by: cans/11-protocol.md#Wire-messages

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Payload structs ──────────────────────────────────────────────
// Referenced by both wire enums and plugin hook signatures.
// One definition, one place.

/// Canonical exec identity: the terminal binary after interpreter
/// unwrapping. Policy matches on this, never on binary_path or argv[0].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedBinary {
    /// Physical filesystem device ID of the resolved executable inode.
    pub dev: u64,
    /// Inode number of the resolved executable binary.
    pub ino: u64,
    /// Raw 32-byte BLAKE3 digest of the resolved executable bytes.
    pub blake3: [u8; 32],
    /// Canonical resolved path of the terminal binary.
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrapEvent {
    pub seq: u64,
    pub pid: u32,
    pub ppid: u32,
    pub task_id: u64,
    pub binary_path: String,
    /// Physical filesystem device ID of the executable inode
    pub binary_dev: u64,
    /// Inode number of executable binary (resists mount/chroot spoofing)
    pub binary_ino: u64,
    /// Raw 32-byte BLAKE3 digest; the receipt carries it as hex String.
    pub binary_blake3: [u8; 32],
    pub argv: Vec<String>,
    pub envp: HashMap<String, String>,
    pub cwd: String,
    /// Terminal binary after interpreter unwrapping; equals the requested
    /// identity when no interpreter layer exists.
    pub resolved_binary: ResolvedBinary,
    /// Binaries traversed while unwrapping, requested first, terminal last.
    pub interpreter_chain: Vec<ResolvedBinary>,
    /// The -c/-e string when an interpreter layer carries one, else None.
    pub shell_payload: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessExit {
    pub pid: u32,
    pub exit_code: i32,
    /// True physical elapsed time from spawn to reap.
    pub wall_clock_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPressureWarning {
    pub available_mb: u32,
    pub total_mb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortBound {
    pub guest_port: u16,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heartbeat {
    pub uptime_ms: u64,
}

// ── Wire enums ───────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub enum GuestToHost {
    TrapEvent(TrapEvent),
    ProcessExit(ProcessExit),
    MemoryPressureWarning(MemoryPressureWarning),
    PortBound(PortBound),
    Heartbeat(Heartbeat),
}

/// Handshake validation on Control transport 0x0000 initialization.
/// Handshake validates version == 0x01; mismatches reject with
/// E_INCOMPATIBLE_SHIM_VERSION. negotiated_capabilities =
/// host.capabilities & guest.capabilities. A missing mandatory feature
/// bit aborts; a missing optional bit disables that interposer path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Handshake {
    pub version: u16, // 0x01
    pub capabilities: u64,
}

/// Stream mux 0x0001, stream kind EXEC 0x0001, sub-lane codes.
/// Framing: stream_id: u32, flags: u8, len: u32, payload. stdout/stderr carry
/// raw bytes; no line wrapping, no escape injection; PTY bypassed.
/// Guest shim holds a dynamic ExecSlot slab mapping guest pid to
/// exec_id, sized min(guest RLIMIT_NPROC, host max_exec_slots),
/// default 4096; slab saturation returns MSG_EXEC_REJECT{ENOMEM}
/// in-band. Unmatched reaped PIDs discard into a zero-allocation sink.
/// Exit frames defer until stdout/stderr EOF flush.
/// Out-of-band isolation: SIGINT and SIGTERM route exclusively over
/// Control transport 0x0000, so signals never stall behind saturated
/// stdout/stderr rings; signals carry exec_id, never raw guest pid.
/// Window resize rides EXEC control sub-lane 0x03.
pub const EXEC_STREAM_STDIN: u8 = 0x00;
pub const EXEC_STREAM_STDOUT: u8 = 0x01;
pub const EXEC_STREAM_STDERR: u8 = 0x02;
pub const EXEC_STREAM_CONTROL: u8 = 0x03; // stream control: exit code, EOF, rlimit descriptors, window resize (signals ride Control 0x0000)

#[derive(Debug, Serialize, Deserialize)]
pub enum HostToGuest {
    /// Verdict on Intercepted Syscall
    TrapDecision {
        seq: u64,
        action: Verdict,
    },
    /// Dynamic Memory Balloon Adjustment
    AdjustBalloon {
        target_mb: u32,
    },
    ChannelData {
        seq: u64,
        payload: Vec<u8>,
    },
}

/// Unified verdict (02-plugin-model, 11-protocol): one enum across
/// Trap, Fs, Net, Host, Console, Exec, MCP, and DB rings. No per-ring aliases.
/// Method mapping: Pass = c.pass(), Patch = c.patch(mutation),
/// Drop = c.drop(errno, reason), Suspend = c.suspend(until),
/// Abort = c.abort(reason).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuspendUntil {
    /// Hold until the given deadline in milliseconds elapses.
    Deadline(u64),
    /// Hold until an external approval arrives.
    Approval,
    /// Hold until the handshake completes.
    Handshake,
}

/// Patch payload, registered per ring. The non-Custom variants carry
/// typed payloads serialized as CBOR per 11-protocol.md Wire-framing.
/// Custom carries name + CBOR; the handler name resolves at SDK
/// registration (see 11-protocol.md Custom dispatch). Unknown names
/// fail with EUnknownAction before the pipeline resumes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchPayload {
    ExecveMutation(ExecveMutation),
    /// Served inode for on_fs_lookup: same fields as FsInodeAttr, carried
    /// on the wire so lookup serve is not confused with write mutation.
    FsInode { mode: u32, uid: u32, gid: u32, size: u64, content: Vec<u8> },
    Custom { name: String, cbor: ciborium::value::Value },
}

/// POSIX errno for a deny verdict, as a typed enum (not String).
/// The set is the live typed enum in
/// crates/kelir-interface/src/errno_types.rs#Errno: 15 POSIX names
/// the storage plane maps in fuse_storage.rs#drop_errno, plus 7
/// non-POSIX policy names the engine rejects via Halt. Traversal
/// denials pin EACCES (POSIX only; no invented errno name).
/// Variant spelling is E + the POSIX name in lower CamelCase
/// (Eperm, Eacces); the wire form is the SCREAMING name, except
/// EConfig, ESecurityViolation, EUnknownKind, EClockRegress,
/// which keep their mixed-case wire spelling. There is no silent
/// fallback: an unrecognized name is not an errno.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Errno {
    Eperm,
    Eacces,
    Enoent,
    Einval,
    Enospc,
    Erofs,
    Econnrefused,
    Enxio,
    Ewouldblock,
    Efault,
    Ebusy,
    Exdev,
    Epipe,
    Enodata,
    Eproto,
    Epolicy,
    Etoolquota,
    Ewxviolation,
    #[serde(rename = "EConfig")]
    Econfig,
    #[serde(rename = "ESecurityViolation")]
    Esecurityviolation,
    #[serde(rename = "EUnknownKind")]
    Eunknownkind,
    #[serde(rename = "EClockRegress")]
    Eclockregress,
}

impl Errno {
    /// Wire form: the SCREAMING POSIX name (e.g. "EPERM").
    pub fn as_str(&self) -> &'static str {
        match self {
            Errno::Eperm => "EPERM",
            Errno::Eacces => "EACCES",
            Errno::Enoent => "ENOENT",
            Errno::Einval => "EINVAL",
            Errno::Enospc => "ENOSPC",
            Errno::Erofs => "EROFS",
            Errno::Econnrefused => "ECONNREFUSED",
            Errno::Enxio => "ENXIO",
            Errno::Ewouldblock => "EWOULDBLOCK",
            Errno::Efault => "EFAULT",
            Errno::Ebusy => "EBUSY",
            Errno::Exdev => "EXDEV",
            Errno::Epipe => "EPIPE",
            Errno::Enodata => "ENODATA",
            Errno::Eproto => "EPROTO",
            Errno::Epolicy => "EPOLICY",
            Errno::Etoolquota => "ETOOLQUOTA",
            Errno::Ewxviolation => "EWXVIOLATION",
            Errno::Econfig => "EConfig",
            Errno::Esecurityviolation => "ESecurityViolation",
            Errno::Eunknownkind => "EUnknownKind",
            Errno::Eclockregress => "EClockRegress",
        }
    }
}

/// Sync hot-path filter result. The buffer is `&mut`; a handler that
/// wants to forward a mutated buffer returns Forward. No Patch variant:
/// "transform" is the buffer write itself, not a separate decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum FilterAction {
    /// Forward the (possibly in-place-mutated) buffer.
    Forward,
    /// Deny. errno surfaces to guest. None = silent drop (chaos only).
    Drop { errno: Option<Errno>, reason: String },
    /// Yield to host.
    Suspend { until: SuspendUntil },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Verdict {
    /// Proceed untouched. Registers and memory are never modified.
    Pass,
    /// In-place mutation; payload schema per ring.
    Patch(PatchPayload),
    /// Deny. errno surfaces to the guest (policy denial is fast-fail,
    /// never silent: L4 TCP gets a TCP RST or ICMP Port Unreachable,
    /// L7 HTTP gets a synthetic 403 with a JSON receipt). errno None
    /// indicates silent blackholing, for plugin-chaos loss injection only.
    Drop { errno: Option<Errno>, reason: String },
    /// Yield or hold to host until `until`.
    Suspend { until: SuspendUntil },
    /// Fatal; pipeline freezes.
    Abort { reason: String },
}
