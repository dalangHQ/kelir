// see: cans/artifacts/code/plugin-trait.rs
use crate::telemetry::{ExecutionReceipt, ExitKind};

#[derive(Debug, Clone)]
pub struct HandlerCtx {
    pub session_id: [u8; 16],
    pub pid: u32,
}

impl HandlerCtx {
    pub fn new(session_id: [u8; 16], pid: u32) -> Self {
        Self { session_id, pid }
    }

    pub fn receipt(&self) -> ExecutionReceipt {
        // TODO: wire to the live per-process receipt in Part 2.
        ExecutionReceipt {
            pid: self.pid,
            binary_path: String::new(),
            binary_blake3: String::new(),
            exit_kind: ExitKind::Normal,
            correlation_token: None,
            dirty_bytes: 0,
            syscall_count: None,
            exit_code: 0,
            wall_clock_ns: 0,
            phase_timings: None,
            egress_bytes: 0,
            egress_dropped_bytes: 0,
            extensions: Vec::new(),
        }
    }
}
