// crates/kelir-core/src/telemetry/receipt.rs
//
// Per-process cryptographic execution receipt. Generated and held by
// kelir-core at the trap/exit boundary. c.receipt() is the single read
// API on HandlerCtx.
//
// Canonical spec home: cans/08-epistemic.md#Execution-receipts

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReceipt {
    pub pid: u32,
    pub binary_path: String,
    /// BLAKE3 digest, canonical 64-char lowercase hex
    pub binary_blake3: String,
    /// True byte count of mutated physical memory.
    /// Hardware-agnostic by design: page size is 16 KB on Apple Silicon
    /// vs 4 KB on Linux/x86, so raw byte counts are the canonical metric.
    pub dirty_bytes: u64,
    /// Total system calls evaluated
    pub syscall_count: u64,
    /// Kernel return code
    pub exit_code: i32,
    /// True physical elapsed time
    pub wall_clock_ns: u64,
    /// Total network egress bytes transmitted
    pub egress_bytes: u64,
}
