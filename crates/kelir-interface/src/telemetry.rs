// see: cans/10-attestation.md#Execution-receipts
// see: cans/10-attestation.md#Causal-delta
use std::path::PathBuf;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExitKind {
    Normal,
    Signal(u32),
    KernelPanic(String),
    HypervisorFault(String),
    DirtyShutdown,
    HardKilled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExecutionReceipt {
    pub pid: u32,
    pub binary_path: String,
    pub binary_blake3: String,
    pub exit_kind: ExitKind,
    pub correlation_token: [u8; 16],
    pub dirty_bytes: u64,
    pub syscall_count: Option<u64>,
    pub exit_code: i32,
    pub wall_clock_ns: u64,
    pub phase_timings: Option<PhaseTimings>,
    pub egress_bytes: u64,
    pub egress_dropped_bytes: u64,
    pub extensions: Vec<TelemetryExtension>,
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CausalDelta {
    pub memory_diff_bytes: u64,
    pub modified_inodes: Vec<PathBuf>,
    pub spawned_pids: Vec<u32>,
    pub egress_bytes: u64,
    pub storage_delta_root: [u8; 32],
    pub receipt_leaf_hash: [u8; 32],
}
