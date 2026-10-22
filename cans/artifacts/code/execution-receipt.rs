// crates/kelir-core/src/telemetry/receipt.rs
//
// Per-process cryptographic execution receipt.
//
// Owned by: cans/08-epistemic.md#Execution-receipts

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReceipt {
    pub pid: u32,
    pub binary_path: String,
    /// BLAKE3 digest, canonical 64-char lowercase hex
    pub binary_blake3: String,
    /// Byte count; always populated.
    /// Byte-not-page reason: see cans/01-architecture.md#VmmDriver-trait
    pub dirty_bytes: u64,
    /// Retired instruction count sampled from host vCPU thread perf counter
    pub retired_instructions: u64,
    /// Populated when ReceiptConfig::include_syscall_count.
    pub syscall_count: Option<u64>,
    /// Kernel return code
    pub exit_code: i32,
    /// True physical elapsed time
    pub wall_clock_ns: u64,
    /// Total network egress bytes transmitted
    pub egress_bytes: u64,
}
