#![forbid(unsafe_code)]

// see: cans/11-protocol.md#Wire-framing
// see: cans/11-protocol.md#Control-channel
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub use crate::errno_types::Errno;

// see: cans/11-protocol.md#Binary-exec-channel
macro_rules! wire_consts {
    ($group:ident, $repr:ty, { $($name:ident = $value:literal),+ $(,)? }) => {
        $( pub const $name: $repr = $value; )+
        pub const $group: &[(&str, $repr)] = &[ $( (stringify!($name), $value) ),+ ];
    };
}

// see: cans/11-protocol.md#Control-channel
macro_rules! wire_enum {
    ($name:ident, $table:ident, { $($variant:ident = $value:literal),+ $(,)? }) => {
        wire_enum! { @decl $name, $table, () { $($variant = $value, )+ } }
    };
    ($name:ident, $table:ident, { default $default:ident = $default_value:literal, $($variant:ident = $value:literal),+ $(,)? }) => {
        wire_enum! { @decl $name, $table, (Default) { #[default] $default = $default_value, $($variant = $value, )+ } }
    };
    (@decl $name:ident, $table:ident, ($($der:ident),*) { $( $(#[$attr:meta])* $variant:ident = $value:literal ),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq $(, $der)* , Serialize, Deserialize)]
        #[repr(u16)]
        pub enum $name {
            $( $(#[$attr])* $variant = $value, )+
        }

        impl $name {
            pub fn from_u16(value: u16) -> Option<Self> {
                match value {
                    $( $value => Some(Self::$variant), )+
                    _ => None,
                }
            }
        }

        pub const $table: &[(&str, u16)] = &[ $( (stringify!($variant), $value) ),+ ];
    };
}

// see: cans/11-protocol.md#Binary-exec-channel
wire_consts! { EXEC_STREAMS, u8, {
    EXEC_STREAM_STDIN = 0x00,
    EXEC_STREAM_STDOUT = 0x01,
    EXEC_STREAM_STDERR = 0x02,
    EXEC_STREAM_CONTROL = 0x03,
}}

// see: cans/11-protocol.md#Control-channel
wire_enum! { StreamKind, STREAM_KINDS, {
    default Exec = 0x0001,
    PtyControl = 0x0002,
    Vfs = 0x0003,
    Bulk = 0x0004,
    L4Forward = 0x0005,
    TerminalConsole = 0x0006,
    ProfileSync = 0x0010,
    DbSqlite = 0x0011,
    McpConsole = 0x0012,
}}

// see: cans/11-protocol.md#Control-channel
wire_enum! { Channel, CHANNELS, {
    Control = 0x0000,
    StreamMux = 0x0001,
}}

// see: cans/11-protocol.md#Wire-framing
wire_enum! { MsgType, MSG_TYPES, {
    Trap = 0x0001,
    ChannelPayload = 0x0002,
    VmLifecycleControl = 0x0003,
    SystemTelemetry = 0x0004,
    Signal = 0x0005,
}}

// see: cans/11-protocol.md#Interposition-contract
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecveMutation {
    pub new_binary: Option<String>,
    pub new_argv: Option<Vec<String>>,
    pub set_env: Vec<(String, String)>,
    #[serde(default)]
    pub unset_env: Vec<String>,
}

// see: cans/11-protocol.md#Interposition-contract
impl ExecveMutation {
    pub fn deduped_set_env(&self) -> Vec<(String, String)> {
        let mut seen: Vec<&str> = Vec::new();
        let mut deduped: Vec<(String, String)> = Vec::new();
        for (key, value) in &self.set_env {
            if seen.contains(&key.as_str()) {
                continue;
            }
            seen.push(key.as_str());
            deduped.push((key.clone(), value.clone()));
        }
        deduped
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PatchPayload {
    ExecveMutation(ExecveMutation),
    FsInode {
        mode: u32,
        uid: u32,
        gid: u32,
        size: u64,
        content: Vec<u8>,
    },
    FsPathRedirect {
        target_path: String,
    },
    // see: cans/artifacts/code/messages.rs#PatchPayload
    NetRedirect {
        target_host: String,
        target_port: u16,
    },
    // see: cans/artifacts/code/messages.rs#PatchPayload
    SignalMutation {
        replacement_signal: i32,
    },
    Custom {
        name: String,
        cbor: ciborium::value::Value,
    },
}

// see: cans/02-plugin-model.md#Unified-verdict
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuspendUntil {
    Deadline(u64),
    Approval,
    Handshake,
    Condition { poll_interval_ms: u64, handle: u64 },
}

// see: cans/08-lifecycle.md#External-suspension
pub const MAX_PENDING_APPROVAL_HOLDS: u32 = 10;
pub const APPROVAL_HOLD_TTL_MS: u64 = 300_000;

// see: cans/08-lifecycle.md#External-suspension
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalHoldConfig {
    pub max_pending_holds: u32,
    pub hold_ttl_ms: u64,
}

impl Default for ApprovalHoldConfig {
    fn default() -> Self {
        Self {
            max_pending_holds: MAX_PENDING_APPROVAL_HOLDS,
            hold_ttl_ms: APPROVAL_HOLD_TTL_MS,
        }
    }
}

// see: cans/08-lifecycle.md#External-suspension
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalHoldTracker {
    pub config: ApprovalHoldConfig,
    pub holds: Vec<u64>,
}

impl ApprovalHoldTracker {
    pub fn new(config: ApprovalHoldConfig) -> Self {
        Self {
            config,
            holds: Vec::new(),
        }
    }

    pub fn is_expired(&self, started_ms: u64, now_ms: u64) -> bool {
        now_ms >= started_ms.saturating_add(self.config.hold_ttl_ms)
    }

    pub fn expire(&mut self, now_ms: u64) {
        let ttl_ms = self.config.hold_ttl_ms;
        self.holds
            .retain(|started_ms| now_ms < started_ms.saturating_add(ttl_ms));
    }

    pub fn pending_count(&self, now_ms: u64) -> usize {
        let ttl_ms = self.config.hold_ttl_ms;
        self.holds
            .iter()
            .filter(|started_ms| now_ms < started_ms.saturating_add(ttl_ms))
            .count()
    }

    pub fn try_hold(&mut self, now_ms: u64) -> bool {
        self.expire(now_ms);
        if self.holds.len() >= self.config.max_pending_holds as usize {
            return false;
        }
        self.holds.push(now_ms);
        true
    }
}

// see: cans/02-plugin-model.md#Unified-verdict
// see: cans/11-protocol.md#Verdict-method-mapping
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Action {
    Pass,
    Forward,
    Drop { errno: Errno, reason: String },
    // see: cans/11-protocol.md#Signal-and-fork-interception
    Absorb,
    Suspend { until: SuspendUntil },
    Abort { reason: String },
    Patch(PatchPayload),
}

impl Action {
    // see: cans/02-plugin-model.md#Unified-verdict
    pub fn guidance(&self) -> Option<&str> {
        match self {
            Action::Drop { reason, .. } | Action::Abort { reason } => Some(reason.as_str()),
            _ => None,
        }
    }
}

// see: cans/02-plugin-model.md#Unified-verdict
pub type Verdict = Action;

// see: cans/02-plugin-model.md#Unified-verdict
pub type FilterAction = Action;

// see: cans/11-protocol.md#Memory-mapping-flags
pub const PROT_NONE: u32 = 0;
pub const PROT_READ: u32 = 1;
pub const PROT_WRITE: u32 = 2;
pub const PROT_EXEC: u32 = 4;

// see: cans/11-protocol.md#Memory-mapping-flags
pub const MFD_CLOEXEC: u32 = 0x0001;
pub const MFD_ALLOW_SEALING: u32 = 0x0002;

// see: cans/11-protocol.md#Interposition-contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrappedSyscall {
    pub nr: i64,
    pub mprotect_prot: Option<u32>,
    pub memfd_create_flags: Option<u32>,
}

// see: cans/artifacts/code/messages.rs#ResolvedBinary
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResolvedBinary {
    pub dev: u64,
    pub ino: u64,
    #[serde(with = "crate::hex_codec::serde_hex_32")]
    pub blake3: [u8; 32],
    pub path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrapEvent {
    pub seq: u64,
    pub pid: u32,
    pub ppid: u32,
    // TODO (L3) [plugin_engine]: fill tree_depth from the live trap path; 0 until the trap path records process depth; slop: tree_depth stays 0; trap depth never recorded.
    #[serde(default)]
    pub tree_depth: u32,
    // see: cans/artifacts/code/messages.rs#TrapEvent — on-the-wire field, consumed there
    pub task_id: u64,
    pub binary_path: String,
    pub binary_dev: u64,
    pub binary_ino: u64,
    // see: cans/artifacts/code/messages.rs#TrapEvent
    // see: cans/11-protocol.md#Wire-framing
    #[serde(with = "crate::hex_codec::serde_hex_32")]
    pub binary_blake3: [u8; 32],
    pub argv: Vec<String>,
    pub envp: HashMap<String, String>,
    pub cwd: String,
    #[serde(default)]
    pub syscall: Option<TrappedSyscall>,
    // see: cans/artifacts/code/messages.rs#TrapEvent
    #[serde(default)]
    pub resolved_binary: ResolvedBinary,
    // see: cans/11-protocol.md#Trap-fast-path — on-the-wire field, consumed there
    #[serde(default)]
    pub interpreter_chain: Vec<ResolvedBinary>,
    // see: cans/11-protocol.md#Trap-fast-path — on-the-wire field, consumed there
    #[serde(default)]
    pub shell_payload: Option<String>,
}

// see: cans/artifacts/code/messages.rs#TrapEvent
impl TrapEvent {
    pub fn new(seq: u64, pid: u32, ppid: u32, binary_path: impl Into<String>) -> Self {
        Self {
            seq,
            pid,
            ppid,
            binary_path: binary_path.into(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessExit {
    pub pid: u32,
    pub exit_code: i32,
    pub wall_clock_ns: u64,
}

// see: cans/artifacts/code/messages.rs#MemoryPressureWarning
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
pub enum GuestToHost {
    TrapEvent(Box<TrapEvent>),
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
pub enum HostToGuest {
    TrapDecision { seq: u64, action: Verdict },
    AdjustBalloon { target_mb: u32 },
    ChannelData { seq: u64, payload: Vec<u8> },
}

// see: cans/11-protocol.md#Trap-fast-path
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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
