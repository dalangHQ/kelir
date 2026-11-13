use std::collections::HashMap;
use std::sync::RwLock;

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

// see: cans/10-attestation.md#Execution-receipts
// see: cans/11-protocol.md#Wire-framing
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

// see: cans/10-attestation.md#Execution-receipts
#[derive(Debug, Default)]
pub struct ReceiptStore {
    receipts: RwLock<HashMap<u32, ExecutionReceipt>>,
}

impl Clone for ReceiptStore {
    fn clone(&self) -> Self {
        let store = Self::new();
        for receipt in self.snapshot() {
            store.insert(receipt);
        }
        store
    }
}

impl ReceiptStore {
    pub fn new() -> Self {
        Self::default()
    }

    // see: cans/10-attestation.md#Execution-receipts
    pub fn insert(&self, receipt: ExecutionReceipt) {
        let mut receipts = self
            .receipts
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        receipts.insert(receipt.pid, receipt);
    }

    // see: cans/10-attestation.md#Execution-receipts
    pub fn lookup(&self, pid: u32) -> Option<ExecutionReceipt> {
        let receipts = self
            .receipts
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        receipts.get(&pid).cloned()
    }

    // see: cans/08-lifecycle.md#CoW-fork
    pub fn snapshot(&self) -> Vec<ExecutionReceipt> {
        let receipts = self
            .receipts
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        receipts.values().cloned().collect()
    }
}

// TODO (L3) [ar_12_session_attestation_token_injection]: consumed by cans/10-attestation.md#Attestation-provenance-classification session-token verification.; slop: token presence asserted, host-key signature never verified
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
