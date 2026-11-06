#![forbid(unsafe_code)]

// crates/kelir-interface/src/protocol.rs
//
// All wire messages exchanged over transport port 5252.
// 5252 is a transport identifier, not an AF_VSOCK port: the Transport
// backend carries it (shared-memory SPSC rings primary; AF_VSOCK on
// Linux/macOS and named pipes on Windows as debug fallbacks).
// Exactly two transports: Control 0x0000 and Stream Mux 0x0001;
// every other capability is a stream kind multiplexed on 0x0001
// (see codec.rs for the kind registry).
// Payloads encode as CBOR inside the TLV frame defined in codec.rs.
//
// Owned by: cans/11-protocol.md#Wire-messages

// see: cans/11-protocol.md#Wire-framing
// see: cans/11-protocol.md#Control-channel
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub const EXEC_STREAM_STDIN: u8 = 0x00;
pub const EXEC_STREAM_STDOUT: u8 = 0x01;
pub const EXEC_STREAM_STDERR: u8 = 0x02;
pub const EXEC_STREAM_CONTROL: u8 = 0x03;

// StreamKind / MsgType mirror the codec.rs const registry
// (TRANSPORT_*/KIND_*/MSG_*); values are authoritative there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum StreamKind {
    Exec = 0x0001,
    PtyControl = 0x0002,
    Vfs = 0x0003,
    Bulk = 0x0004,
    L4Forward = 0x0005,
    TerminalConsole = 0x0006,
    ProfileSync = 0x0010,
    DbSqlite = 0x0011,
    McpConsole = 0x0012,
}

impl StreamKind {
    pub fn from_u16(v: u16) -> Option<Self> {
        match v {
            0x0001 => Some(Self::Exec),
            0x0002 => Some(Self::PtyControl),
            0x0003 => Some(Self::Vfs),
            0x0004 => Some(Self::Bulk),
            0x0005 => Some(Self::L4Forward),
            0x0006 => Some(Self::TerminalConsole),
            0x0010 => Some(Self::ProfileSync),
            0x0011 => Some(Self::DbSqlite),
            0x0012 => Some(Self::McpConsole),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum MsgType {
    Trap = 0x0001,
    ChannelPayload = 0x0002,
    VmLifecycleControl = 0x0003,
    SystemTelemetry = 0x0004,
}

impl MsgType {
    pub fn from_u16(v: u16) -> Option<Self> {
        match v {
            0x0001 => Some(Self::Trap),
            0x0002 => Some(Self::ChannelPayload),
            0x0003 => Some(Self::VmLifecycleControl),
            0x0004 => Some(Self::SystemTelemetry),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecveMutation {
    pub new_binary: Option<String>,
    pub new_argv: Option<Vec<String>>,
    pub set_env: Vec<(String, String)>,
    pub unset_env: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PatchPayload {
    ExecveMutation(Vec<u8>),
    /// Served inode for on_fs_lookup: same fields as FsInodeAttr, carried
    /// on the wire so lookup serve is not confused with write mutation.
    FsInode {
        mode: u32,
        uid: u32,
        gid: u32,
        size: u64,
        content: Vec<u8>,
    },
    /// Custom carries name + CBOR; the handler name resolves at SDK
    /// registration (see 11-protocol.md Custom dispatch). Unknown names
    /// fail with EUnknownAction before the pipeline resumes.
    Custom {
        name: String,
        cbor: ciborium::value::Value,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SuspendUntil {
    /// Hold until the given deadline in milliseconds elapses.
    Deadline(u64),
    /// Hold until an external approval arrives.
    Approval,
    /// Hold until the handshake completes.
    Handshake,
}

// see: cans/02-plugin-model.md#Unified-verdict
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    Drop {
        errno: Option<String>,
        reason: String,
    },
    /// Yield or hold to host until `until`.
    Suspend { until: SuspendUntil },
    /// Fatal; pipeline freezes.
    Abort { reason: String },
}

// see: cans/02-plugin-model.md#Unified-verdict
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum FilterAction {
    /// Forward the (possibly in-place-mutated) buffer.
    Forward,
    /// Deny. errno surfaces to guest. None = silent drop (chaos only).
    Drop {
        errno: Option<String>,
        reason: String,
    },
    /// Yield to host.
    Suspend { until: SuspendUntil },
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessExit {
    pub pid: u32,
    pub exit_code: i32,
    /// True physical elapsed time from spawn to reap.
    pub wall_clock_ns: u64,
}

// see: cans/artifacts/code/messages.rs
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

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
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

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessLimits {
    pub max_as_bytes: Option<u64>,
    pub max_nofile: Option<u64>,
    pub max_nproc: Option<u64>,
    pub max_cpu_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PtyControlMessage {
    SignalForward { signal: u32, exec_id: u64 },
}
