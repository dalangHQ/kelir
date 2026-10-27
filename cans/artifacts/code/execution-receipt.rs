// crates/kelir-interface/src/telemetry/receipt.rs
//
// Per-process cryptographic execution receipt.
//
// Owned by: cans/08-epistemic.md#Execution-receipts

use serde::{Deserialize, Serialize};

/// Plugin-scoped telemetry entry. Core never inspects `payload`;
/// `namespace` is the producing plugin's taxonomy boundary URN.
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReceipt {
    pub pid: u32,
    pub binary_path: String,
    /// BLAKE3 digest, canonical 64-char lowercase hex
    pub binary_blake3: String,
    /// Correlation token for model-generation-to-trap attribution
    pub correlation_token: Option<String>,
    /// Hypervisor dirty-byte counter; always populated.
    /// Byte-not-page reason: see cans/01-architecture.md#VmmDriver-trait
    pub dirty_bytes: u64,
    /// Bytes dirtied strictly within target workspace inodes (virtio-fs journal)
    pub workspace_dirty_bytes: u64,
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
    /// Total network egress bytes transmitted
    pub egress_bytes: u64,
    /// Plugin-scoped extensions. Core treats entries as opaque;
    /// empty for sessions with no extension-producing plugin.
    pub extensions: Vec<TelemetryExtension>,
}
