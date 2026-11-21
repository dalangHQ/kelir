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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsVfsExt {
    pub served_inodes: u32,
    pub roots: Vec<String>,
}

// see: cans/10-attestation.md#Hook-trace
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VerdictKind {
    Pass,
    Forward,
    Drop,
    Suspend,
    Abort,
    Patch,
    Absorb,
}

// see: cans/10-attestation.md#Hook-trace
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookSpan {
    pub plugin: String,
    pub ring: crate::fusion_plugin::TrampolineRing,
    pub verdict_kind: VerdictKind,
    pub duration_ns: u64,
    pub ordinal: u32,
}

// see: cans/10-attestation.md#Hook-trace
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookPolicyDescriptor {
    pub plugin: String,
    pub ring: String,
    pub verdict_kind: VerdictKind,
    pub ordinal: u32,
}

impl HookPolicyDescriptor {
    // see: cans/10-attestation.md#Hook-trace
    pub fn from_span(span: &HookSpan) -> Self {
        Self {
            plugin: span.plugin.clone(),
            ring: span.ring.as_tag().to_string(),
            verdict_kind: span.verdict_kind,
            ordinal: span.ordinal,
        }
    }
}

// see: cans/10-attestation.md#Hook-trace
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaskedRecord {
    pub record: String,
    pub pre_redaction_blake3: Option<String>,
    pub masked_bytes: u64,
}

// see: cans/10-attestation.md#Step-evaluation-loop
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepLineageEntry {
    pub node_id: u64,
    pub parent_node_id: Option<u64>,
    pub reviewer_id: Option<String>,
    pub reviewer_signature: Option<String>,
}

// see: cans/10-attestation.md#Attestation-provenance-classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AttestationClass {
    Class1,
    Class2,
    Class3,
}

impl AttestationClass {
    // see: cans/10-attestation.md#Attestation-provenance-classification
    pub fn hardware_anchored() -> Self {
        Self::Class1
    }

    // see: cans/10-attestation.md#Attestation-provenance-classification
    pub fn rootless_container() -> Self {
        Self::Class2
    }

    // see: cans/10-attestation.md#Attestation-provenance-classification
    pub fn satisfies(&self, required: Self) -> bool {
        (*self as u8) <= (required as u8)
    }
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
    #[serde(default)]
    pub hook_trace: Vec<HookSpan>,
    // see: cans/10-attestation.md#Hook-trace
    #[serde(default)]
    pub hook_policy_descriptors: Vec<HookPolicyDescriptor>,
    // see: cans/10-attestation.md#Hook-trace
    #[serde(default)]
    pub masked_records: Vec<MaskedRecord>,
    // see: cans/10-attestation.md#Step-evaluation-loop
    #[serde(default)]
    pub step_lineage: Vec<StepLineageEntry>,
    #[serde(skip)]
    pub fs_vfs_ext: Option<FsVfsExt>,
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

// TODO: [ar_12_session_attestation_token_injection] fill in the L3 wave; consumed by cans/10-attestation.md#Attestation-provenance-classification session-token verification.; slop: token presence asserted, host-key signature never verified
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestationToken {
    pub session_token: String,
    pub vmlinux_blake3: String,
    pub erofs_blake3: String,
}

impl AttestationToken {
    // see: cans/10-attestation.md#Attestation-provenance-classification
    pub fn class(&self) -> AttestationClass {
        AttestationClass::Class1
    }
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
            hook_trace: _,
            hook_policy_descriptors,
            masked_records,
            step_lineage,
            fs_vfs_ext: _,
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
            hook_trace: _,
            hook_policy_descriptors: other_descriptors,
            masked_records: other_masked,
            step_lineage: other_lineage,
            fs_vfs_ext: _,
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
            && hook_policy_descriptors == other_descriptors
            && masked_records == other_masked
            && step_lineage == other_lineage
    }

    // see: cans/10-attestation.md#Hook-trace
    pub fn hooks(&self) -> &[HookSpan] {
        &self.hook_trace
    }

    // see: cans/10-attestation.md#Hook-trace
    pub fn hook_policy_descriptors(&self) -> &[HookPolicyDescriptor] {
        &self.hook_policy_descriptors
    }

    // see: cans/10-attestation.md#Hook-trace
    pub fn masked_records(&self) -> &[MaskedRecord] {
        &self.masked_records
    }

    // see: cans/10-attestation.md#Hook-trace
    pub fn masked_records_complete(&self) -> bool {
        self.masked_records
            .iter()
            .all(|record| record.pre_redaction_blake3.is_some())
    }

    // see: cans/10-attestation.md#Step-evaluation-loop
    pub fn step_lineage(&self) -> &[StepLineageEntry] {
        &self.step_lineage
    }

    // see: cans/10-attestation.md#Typed-extensions
    pub fn fs_vfs(&self) -> Option<&FsVfsExt> {
        self.fs_vfs_ext.as_ref()
    }

    // see: cans/10-attestation.md#Typed-extensions
    pub fn ext_raw(&self) -> Vec<(String, Vec<u8>)> {
        self.extensions
            .iter()
            .map(|ext| (ext.namespace.clone(), ext.payload.clone()))
            .collect()
    }
}
