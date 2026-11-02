// crates/kelir-interface/src/protocol.rs
//
// All wire messages exchanged over transport port 5252 (retained).
// 5252 is a transport identifier, not an AF_VSOCK port: the Transport
// backend carries it (shared-memory SPSC rings primary; AF_VSOCK on
// Linux/macOS and named pipes on Windows as debug fallbacks).
// Exactly two transports: Control 0x0000 and Stream Mux 0x0001;
// every other capability is a stream kind multiplexed on 0x0001
// (see wire-protocol.rs for the kind registry).
// Payloads encode as CBOR inside the TLV frame defined in codec.rs.
//
// Owned by: cans/11-protocol.md#Wire-messages

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Payload structs ──────────────────────────────────────────────
// Referenced by both wire enums and plugin hook signatures.
// One definition, one place.

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

/// Version negotiation on Control transport 0x0000 initialization. Host and guest
/// exchange Handshakes; negotiated_capabilities = host.capabilities &
/// guest.capabilities. A missing mandatory feature bit aborts with
/// E_INCOMPATIBLE_SHIM_VERSION; a missing optional bit disables that
/// interposer path gracefully. Min version 0x04; no downgrade path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Handshake {
    pub version: u16, // 0x04
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
/// Trap, Fs, Net, Console, Exec, MCP, and DB rings. No per-ring aliases.
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

/// Patch payload, registered per ring. The non-Custom variants carry the
/// ring payload as CBOR bytes; no field layout is pinned here.
/// Custom carries name + CBOR; the handler name resolves at SDK
/// registration (see 11-protocol.md Custom dispatch). Unknown names
/// fail with EUnknownAction before the pipeline resumes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchPayload {
    ExecveMutation(Vec<u8>),
    HttpFrame(Vec<u8>),
    FsChunk(Vec<u8>),
    /// Served inode for on_fs_lookup: same fields as FsInodeAttr, carried
    /// on the wire so lookup serve is not confused with write mutation.
    FsInode { mode: u32, uid: u32, gid: u32, size: u64, content: Vec<u8> },
    JsonRpcFrame(Vec<u8>),
    PgFrame(Vec<u8>),
    Custom { name: String, cbor: ciborium::value::Value },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    /// Proceed untouched. Registers and memory are never modified.
    Pass,
    /// In-place mutation; payload schema per ring.
    Patch(PatchPayload),
    /// Deny. errno surfaces to the guest (policy denial is fast-fail,
    /// never silent: L4 TCP gets a TCP RST or ICMP Port Unreachable,
    /// L7 HTTP gets a synthetic 403 with a JSON receipt). errno None
    /// indicates silent blackholing, for plugin-chaos loss injection only.
    Drop { errno: Option<String>, reason: String },
    /// Yield or hold to host until `until`.
    Suspend { until: SuspendUntil },
    /// Fatal; pipeline freezes.
    Abort { reason: String },
}
