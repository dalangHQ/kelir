#![forbid(unsafe_code)]

// see: cans/11-protocol.md#Wire-framing
// see: cans/11-protocol.md#Control-channel
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub const MAX_FRAME_PAYLOAD: usize = 65536;
pub const MAX_CONTROL_PAYLOAD: usize = 4096;
pub const MAX_STREAM_ID: u32 = 0x00FF_FFFF;
pub const PORT_CONTROL: u16 = 0x0000;
pub const PORT_MUX: u16 = 0x0001;
pub const TRANSPORT_PORT: u32 = 5252;

pub const FLAG_OPEN: u8 = 0x01;
pub const FLAG_DATA: u8 = 0x02;
pub const FLAG_CLOSE: u8 = 0x04;
pub const FLAG_RST: u8 = 0x08;
pub const FLAG_WINDOW_UPDATE: u8 = 0x10;

pub const EXEC_STREAM_STDIN: u8 = 0x00;
pub const EXEC_STREAM_STDOUT: u8 = 0x01;
pub const EXEC_STREAM_STDERR: u8 = 0x02;
pub const EXEC_STREAM_CONTROL: u8 = 0x03;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MuxFrameHeader {
    pub stream_id: u32,
    pub flags: u8,
    pub len: u32,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum MsgType {
    Trap = 0x0001,
    ChannelPayload = 0x0002,
    VmLifecycleControl = 0x0003,
    SystemTelemetry = 0x0004,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessExit {
    pub pid: u32,
    pub exit_code: i32,
    pub wall_clock_ns: u64,
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
