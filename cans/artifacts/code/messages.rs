// crates/kelir-interface/src/protocol.rs
//
// All wire messages exchanged over vsock:5252. Payloads encode as CBOR
// inside the TLV frame defined in codec.rs.
//
// Owned by: cans/09-protocol.md#Wire-messages

use ciborium::value::Value as CborValue;
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
    /// Anti-cyclic rewrite counter. Exceeding 3 aborts task with ECYCLICREWRITE.
    pub rewrite_count: u8,
    pub binary_path: String,
    /// Physical filesystem device ID of the executable inode
    pub binary_dev: u64,
    /// Inode number of executable binary (resists mount/chroot spoofing)
    pub binary_ino: u64,
    pub binary_blake3: [u8; 32],
    pub argv: Vec<String>,
    pub envp: HashMap<String, String>,
    pub cwd: String,
    /// Process namespace depth; >1 indicates nested container execution
    pub namespace_depth: u8,
    pub ancestor_chain: Vec<String>,
    pub loaded_libraries: Vec<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowResize {
    pub rows: u16,
    pub cols: u16,
    pub xpixel: u16,
    pub ypixel: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendSignal {
    pub pid: u32,
    pub signal: u32,
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

#[derive(Debug, Serialize, Deserialize)]
pub enum HostToGuest {
    /// Decision on Intercepted Syscall
    TrapDecision {
        seq: u64,
        action: TrapAction,
    },
    WindowResize(WindowResize),
    SendSignal(SendSignal),
    /// Dynamic Memory Balloon Adjustment
    AdjustBalloon {
        target_mb: u32,
    },
    /// Extension frame on a host-registered channel kind.
    /// `kind` resolution: see: cans/09-protocol.md#Channel-registration
    /// Unknown kinds abort delivery with EUnknownChannel.
    ChannelData {
        kind: u16,
        seq: u64,
        payload: Vec<u8>,
    },
}

/// Permission lattice (monotonic): Allow < Rewrite < Deny < Abort.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrapAction {
    /// Resume the syscall untouched. Registers and memory are never modified.
    Allow,
    /// Hypervisor cgroup freeze: freeze guest task group, return control
    /// cleanly to host event loop
    Yield {
        checkpoint_id: String,
    },
    /// Immediate hypervisor-level uncatchable termination of guest workload
    Abort {
        exit_code: i32,
        reason: String,
    },
    /// Inject exit code and stderr into the agent's TTY.
    Deny {
        exit_code: i32,
        stderr_message: String,
    },
    /// See cans/09-protocol.md#Env-injection
    /// Injected env keys must remain disjoint across handlers; collisions raise EAmbiguousMutation.
    Rewrite {
        new_binary: String,
        new_argv: Vec<String>,
        injected_env: HashMap<String, String>,
    },
    /// Plugin-initiated hold: freeze vCPUs, flush RAM, poll for release.
    /// Reason attribution is plugin-side; core sees only the hold.
    HostHold {
        poll_interval_ms: u32,
    },
    /// Extension point. Dispatched by name to a registered handler;
    /// unknown names fail with EUnknownAction before the pipeline resumes.
    /// Closes the fixed-variant lock-in (quarantine, throttle, shadow-route).
    Custom(CborValue),
}
