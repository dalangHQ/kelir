#![forbid(unsafe_code)]

// see: cans/11-protocol.md#Wire-framing
// see: cans/11-protocol.md#Control-channel
use std::collections::HashMap;

use serde::de::{Deserializer, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use std::fmt;

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
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[repr(u16)]
        pub enum $name {
            $( $variant = $value, )+
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
    Exec = 0x0001,
    PtyControl = 0x0002,
    Vfs = 0x0003,
    Bulk = 0x0004,
    L4Forward = 0x0005,
    TerminalConsole = 0x0006,
    ProfileSync = 0x0010,
    DbSqlite = 0x0011,
    McpConsole = 0x0012,
}}

impl Default for StreamKind {
    #[allow(clippy::derivable_impls)]
    fn default() -> Self {
        Self::Exec
    }
}

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

struct EnvPairsVisitor;

impl<'de> Visitor<'de> for EnvPairsVisitor {
    type Value = Vec<(String, String)>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a map or a sequence of key/value pairs")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut pairs = Vec::new();
        while let Some(pair) = seq.next_element::<(String, String)>()? {
            pairs.push(pair);
        }
        Ok(pairs)
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut pairs = Vec::new();
        while let Some((key, value)) = map.next_entry::<String, String>()? {
            pairs.push((key, value));
        }
        Ok(pairs)
    }
}

// see: cans/plugins/tool-guard.md#Total-tool-call-interposition
fn deserialize_env_pairs<'de, D>(deserializer: D) -> Result<Vec<(String, String)>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_any(EnvPairsVisitor)
}

// see: cans/plugins/tool-guard.md#Total-tool-call-interposition
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecveMutation {
    #[serde(alias = "binary")]
    pub new_binary: Option<String>,
    #[serde(alias = "argv")]
    pub new_argv: Option<Vec<String>>,
    #[serde(default, alias = "env", deserialize_with = "deserialize_env_pairs")]
    pub set_env: Vec<(String, String)>,
    #[serde(default)]
    pub unset_env: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PatchPayload {
    ExecveMutation(ExecveMutation),
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
    Abort {
        reason: String,
    },
}

// see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
pub const PROT_NONE: u32 = 0;
pub const PROT_READ: u32 = 1;
pub const PROT_WRITE: u32 = 2;
pub const PROT_EXEC: u32 = 4;

// see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
pub const MFD_CLOEXEC: u32 = 0x0001;
pub const MFD_ALLOW_SEALING: u32 = 0x0002;

// see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrappedSyscall {
    pub nr: i64,
    pub mprotect_prot: Option<u32>,
    pub memfd_create_flags: Option<u32>,
}

// see: cans/artifacts/code/messages.rs#TrapEvent
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResolvedBinary {
    pub dev: u64,
    pub ino: u64,
    pub blake3: [u8; 32],
    pub path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrapEvent {
    pub seq: u64,
    pub pid: u32,
    pub ppid: u32,
    pub task_id: u64,
    pub binary_path: String,
    pub binary_dev: u64,
    pub binary_ino: u64,
    // see: cans/artifacts/code/messages.rs#TrapEvent
    pub binary_blake3: [u8; 32],
    pub argv: Vec<String>,
    pub envp: HashMap<String, String>,
    pub cwd: String,
    #[serde(default)]
    pub syscall: Option<TrappedSyscall>,
    // see: cans/artifacts/code/messages.rs#TrapEvent
    #[serde(default)]
    pub resolved_binary: ResolvedBinary,
    #[serde(default)]
    pub interpreter_chain: Vec<ResolvedBinary>,
    #[serde(default)]
    pub shell_payload: Option<String>,
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
#[non_exhaustive]
#[allow(clippy::large_enum_variant)]
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
