use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptExtension {
    pub namespace: String,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseTimings {
    pub trap_ingress_ns: u64,
    pub ffi_hop_ns: u64,
    pub policy_ns: u64,
    pub verdict_egress_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct ExecutionReceipt {
    pub pid: u32,
    pub binary_path: String,
    pub binary_blake3: String,
    pub exit_kind: ExitKind,
    pub correlation_token: Option<String>,
    pub dirty_bytes: u64,
    pub syscall_count: Option<u64>,
    pub exit_code: i32,
    pub wall_clock_ns: u64,
    pub phase_timings: Option<PhaseTimings>,
    pub egress_bytes: u64,
    pub egress_dropped_bytes: u64,
    pub extensions: Vec<ReceiptExtension>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestationToken {
    pub session_token: String,
    pub vmlinux_blake3: String,
    pub erofs_blake3: String,
}

impl ExecutionReceipt {
    // see: cans/10-attestation.md#Execution-receipts
    pub fn same_identity(&self, other: &Self) -> bool {
        let Self {
            pid,
            binary_path,
            binary_blake3,
            exit_kind,
            correlation_token,
            dirty_bytes,
            syscall_count,
            exit_code,
            wall_clock_ns: _,
            phase_timings: _,
            egress_bytes,
            egress_dropped_bytes,
            extensions,
        } = self;
        let Self {
            pid: other_pid,
            binary_path: other_binary_path,
            binary_blake3: other_binary_blake3,
            exit_kind: other_exit_kind,
            correlation_token: other_correlation_token,
            dirty_bytes: other_dirty_bytes,
            syscall_count: other_syscall_count,
            exit_code: other_exit_code,
            wall_clock_ns: _,
            phase_timings: _,
            egress_bytes: other_egress_bytes,
            egress_dropped_bytes: other_egressed_bytes,
            extensions: other_extensions,
        } = other;
        pid == other_pid
            && binary_path == other_binary_path
            && binary_blake3 == other_binary_blake3
            && exit_kind == other_exit_kind
            && correlation_token == other_correlation_token
            && dirty_bytes == other_dirty_bytes
            && syscall_count == other_syscall_count
            && exit_code == other_exit_code
            && egress_bytes == other_egress_bytes
            && egress_dropped_bytes == other_egressed_bytes
            && extensions == other_extensions
    }
}
