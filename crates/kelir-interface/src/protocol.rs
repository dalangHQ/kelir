#![forbid(unsafe_code)]

// see: cans/11-protocol.md#Wire-framing
// see: cans/11-protocol.md#Control-channel
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub const EXEC_STREAM_STDIN: u8 = 0x00;
pub const EXEC_STREAM_STDOUT: u8 = 0x01;
pub const EXEC_STREAM_STDERR: u8 = 0x02;
pub const EXEC_STREAM_CONTROL: u8 = 0x03;

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
    FsInode {
        mode: u32,
        uid: u32,
        gid: u32,
        size: u64,
        content: Vec<u8>,
    },
    Custom {
        name: String,
        cbor: ciborium::value::Value,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SuspendUntil {
    Deadline(u64),
    Approval,
    Handshake,
}

// see: cans/02-plugin-model.md#Unified-verdict
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Verdict {
    Pass,
    Patch(PatchPayload),
    Drop {
        errno: Option<String>,
        reason: String,
    },
    Suspend {
        until: SuspendUntil,
    },
    Abort {
        reason: String,
    },
}

// see: cans/02-plugin-model.md#Unified-verdict
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum FilterAction {
    Forward,
    Drop {
        errno: Option<String>,
        reason: String,
    },
    Suspend {
        until: SuspendUntil,
    },
}

// see: cans/plugins/supply-chain.md
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyscallTrap {
    pub syscall_nr: u64,
    pub prot_flags: Option<u32>,
    pub memfd_flags: Option<u32>,
}

// TODO (L3): carry SyscallTrap on TrapEvent once the TrapEvent literal in crates/kelir/src/handler_ctx.rs gains the field; that file is outside fix2-part5 scope.
pub const PROT_EXEC: u32 = 0x4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrapEvent {
    pub seq: u64,
    pub pid: u32,
    pub ppid: u32,
    pub task_id: u64,
    pub binary_path: String,
    pub binary_dev: u64,
    pub binary_ino: u64,
    pub binary_blake3: [u8; 32],
    pub argv: Vec<String>,
    pub envp: HashMap<String, String>,
    pub cwd: String,
}

// see: cans/01-architecture.md#Early-boot-diagnostics
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ExitReason {
    KernelPanic(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessExit {
    pub pid: u32,
    pub exit_code: i32,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Handshake {
    pub version: u16,
    pub capabilities: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum HostToGuest {
    TrapDecision { seq: u64, action: Verdict },
    AdjustBalloon { target_mb: u32 },
    ChannelData { seq: u64, payload: Vec<u8> },
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
