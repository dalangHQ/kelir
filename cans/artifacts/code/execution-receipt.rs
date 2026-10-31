// crates/kelir-interface/src/telemetry/receipt.rs
//
// Per-process cryptographic execution receipt.
//
// Owned by: cans/10-epistemic.md#Execution-receipts

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryExtension {
    pub namespace: String,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseTimings {
    pub trap_ingress_ns: u64,
    pub ffi_hop_ns: u64,
    pub policy_ns: u64,
    pub verdict_egress_ns: u64,
}

/// Boot-failure classification. Always populated; replaces reliance on
/// exit_code alone. KernelPanic payloads are scraped from the fixed 64KB
/// early-boot UART ring buffer in host memory (no guest cooperation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExitKind {
    Normal,
    Signal(u32),
    KernelPanic(String),
    HypervisorFault(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReceipt {
    pub pid: u32,
    pub binary_path: String,
    /// Trusted-prefix fast path: executables under an EROFS trusted prefix
    /// (/usr/bin, /bin, /usr/lib) resolve through a pre-computed BLAKE3
    /// table in host RAM via O(1) index (no file read at trap time); the
    /// identity is still recorded here per receipt.
    pub binary_blake3: String,
    pub exit_kind: ExitKind,
    pub correlation_token: Option<String>,
    /// Hypervisor dirty-byte counter; always populated.
    /// Byte-not-page reason: see cans/01-architecture.md#VmmDriver-trait
    pub dirty_bytes: u64,
    /// Cryptographic hardware attestation signature issued by hypervisor
    pub hardware_attestation: [u8; 32],
    /// PMU retired instruction count sampled from host vCPU thread perf counter
    pub retired_instructions: u64,
    /// Populated when ReceiptConfig::include_syscall_count.
    pub syscall_count: Option<u64>,
    /// Kernel return code
    pub exit_code: i32,
    /// True physical elapsed time
    pub wall_clock_ns: u64,
    /// Optional nano-tracer sub-microsecond breakdown
    pub phase_timings: Option<PhaseTimings>,
    /// Total network egress bytes transmitted.
    /// In-guest bytes pre-ingress are not counted; bytes dropped by the
    /// sinkhole or by Reject are counted separately here.
    pub egress_bytes: u64,
    pub egress_dropped_bytes: u64,
    // NOTE: per-command rlimits from the Channel 0x0003 exec request
    // (max_as_bytes, max_nofile, max_nproc, max_cpu_seconds) are
    // reported in the receipt for auditability. The field name for the
    // limits block is not pinned in spec yet — TBD, do not invent one.
    /// Extension entries. Core treats entries as opaque;
    /// empty for sessions with no extension producer.
    pub extensions: Vec<TelemetryExtension>,
}
