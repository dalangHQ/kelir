// crates/kelir-interface/src/telemetry/receipt.rs
//
// Per-process cryptographic execution receipt.
//
// Owned by: cans/10-attestation.md#Execution-receipts

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
    /// Graceful-stop timeout expiry (08-lifecycle Pre-teardown-flush).
    DirtyShutdown,
    /// session.kill() abrupt path (08-lifecycle Pre-teardown-flush).
    HardKilled,
}

/// PMU counters are diagnostic gauges only, emitted to OTel; they never
/// appear in receipts, CausalDelta, or Merkle leaves (the former
/// retired_instructions field was deleted on that basis; no other
/// artifact file referenced it).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReceipt {
    pub pid: u32,
    pub binary_path: String,
    /// Trusted-prefix fast path: executables under an EROFS trusted prefix
    /// (/usr/bin, /bin, /usr/lib) resolve through a pre-computed BLAKE3
    /// table in host RAM via O(1) index (no file read at trap time); the
    /// identity is still recorded here per receipt. The skip requires the
    /// target script and argument paths to resolve strictly within EROFS;
    /// dynamic interpreters (python, node, bash, sh, perl, ruby) inspect
    /// argv[1..], and arguments targeting /workspace/** forfeit the skip
    /// to Gate 3.
    pub binary_blake3: String,
    pub exit_kind: ExitKind,
    pub correlation_token: Option<String>,
    /// Hypervisor dirty-byte counter; always populated.
    /// Byte-not-page reason: see cans/01-architecture.md#VmmDriver-trait
    pub dirty_bytes: u64,
    /// Populated when ReceiptConfig::include_syscall_count.
    pub syscall_count: Option<u64>,
    /// Kernel return code
    pub exit_code: i32,
    /// True physical elapsed time
    pub wall_clock_ns: u64,
    /// Optional nano-tracer sub-microsecond breakdown. Diagnostic only:
    /// emitted to OTel, excluded from Merkle leaves and from receipt
    /// identity; no field here claims cross-host counter equivalence.
    pub phase_timings: Option<PhaseTimings>,
    /// Total network egress bytes transmitted.
    /// In-guest bytes pre-ingress are not counted; bytes dropped by the
    /// sinkhole or by Verdict::Drop are counted separately here.
    pub egress_bytes: u64,
    pub egress_dropped_bytes: u64,
    // NOTE: per-command rlimits from the stream mux 0x0001, stream kind
    // EXEC 0x0001, control sub-lane 0x03 exec request
    // (max_as_bytes, max_nofile, max_nproc, max_cpu_seconds) are
    // reported in the receipt for auditability. The field name for the
    // limits block is not pinned in spec yet — TBD, do not invent one.
    /// Extension entries. Core treats entries as opaque;
    /// empty for sessions with no extension producer.
    pub extensions: Vec<TelemetryExtension>,
}

/// Attestation token: a separate fact emission per 10-attestation
/// (TrapEvent, ExecutionReceipt, CausalDelta, AttestationToken). It
/// is not a receipt field. Its fields are exactly the Class 1
/// hardware-anchored primitives named in 10: session token, vmlinux
/// BLAKE3, EROFS BLAKE3. No other fields are spec-pinned.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestationToken {
    pub session_token: String,
    pub vmlinux_blake3: String,
    pub erofs_blake3: String,
}
