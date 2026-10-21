// crates/kelir-core/src/protocol/messages.rs
//
// All wire messages exchanged over vsock:5252. Guest↔host control/trap plane.
// Payloads encode as CBOR inside the TLV frame defined in codec.rs.
//
// Canonical spec home: cans/09-protocol.md#Wire-messages

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub enum GuestToHost {
    /// Syscall Trap Interception
    TrapEvent {
        seq: u64,
        pid: u32,
        ppid: u32,
        binary_path: String,
        binary_blake3: [u8; 32],
        argv: Vec<String>,
        envp: HashMap<String, String>,
        cwd: String,
        ancestor_chain: Vec<String>,
        entropy: f64,
        loaded_libraries: Vec<String>,
    },
    /// Process Exit Notification
    ProcessExit {
        pid: u32,
        exit_code: i32,
        wall_clock_ns: u64,
    },
    /// Memory Pressure Alert from In-Guest Watcher
    MemoryPressureWarning {
        available_mb: u32,
        total_mb: u32,
    },
    /// Background Service Bound Port
    PortBound {
        guest_port: u16,
        protocol: String,
    },
    Heartbeat {
        uptime_ms: u64,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum HostToGuest {
    /// Decision on Intercepted Syscall
    TrapDecision {
        seq: u64,
        action: TrapAction,
    },
    /// Forwarding TTY Terminal Window Resize (SIGWINCH)
    WindowResize {
        rows: u16,
        cols: u16,
        xpixel: u16,
        ypixel: u16,
    },
    /// Signal Propagation (SIGINT, SIGTERM, SIGKILL)
    SendSignal {
        pid: u32,
        signal: u32,
    },
    /// Dynamic Memory Balloon Adjustment
    AdjustBalloon {
        target_mb: u32,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TrapAction {
    /// Resume the syscall untouched. Registers and memory are never modified.
    Allow,
    /// Inject exit code and stderr into the agent's TTY.
    Deny {
        exit_code: i32,
        stderr_message: String,
    },
    /// Re-execute with substitutions. This is the ONLY path by which
    /// handler-side environment changes reach the guest: a handler that
    /// returns `Decision.resume()` after `c.env.set(..)` is auto-converted
    /// to `Rewrite { new_binary = original, new_argv = original,
    /// injected_env = accumulated }`. See cans/09-protocol.md#Env-injection.
    Rewrite {
        new_binary: String,
        new_argv: Vec<String>,
        injected_env: HashMap<String, String>,
        /// Mandatory terminal disclosure printed to the workload's PTY.
        /// Guarantees zero silent state divergence.
        tty_notice: Option<String>,
    },
    SuspendForApproval {
        poll_interval_ms: u32,
    },
}
