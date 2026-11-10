// see: cans/14-observability.md#Metrics-pipeline
use kelir_interface::telemetry::{CausalDelta, ExecutionReceipt, PhaseTimings};

// see: cans/14-observability.md#Metrics-pipeline
fn paths_to_strings(paths: &[std::path::PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}

// see: cans/14-observability.md#Metrics-pipeline
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtlpReceiptRecord {
    pub pid: u32,
    pub binary_path: String,
    pub binary_blake3: String,
    pub exit_code: i32,
    pub wall_clock_ns: u64,
    pub spans: Vec<crate::OtlpSpan>,
}

// see: cans/14-observability.md#Metrics-pipeline
impl OtlpReceiptRecord {
    pub fn from_receipt(receipt: &ExecutionReceipt) -> Self {
        Self {
            pid: receipt.pid,
            binary_path: receipt.binary_path.clone(),
            binary_blake3: receipt.binary_blake3.clone(),
            exit_code: receipt.exit_code,
            wall_clock_ns: receipt.wall_clock_ns,
            spans: crate::phase_spans(receipt.phase_timings.as_ref()),
        }
    }
}

// see: cans/14-observability.md#Metrics-pipeline
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OtlpPhaseTimingRecord {
    pub trap_ingress_ns: u64,
    pub ffi_hop_ns: u64,
    pub policy_ns: u64,
    pub verdict_egress_ns: u64,
}

// see: cans/14-observability.md#Metrics-pipeline
impl OtlpPhaseTimingRecord {
    pub fn from_timings(timings: &PhaseTimings) -> Self {
        Self {
            trap_ingress_ns: timings.trap_ingress_ns,
            ffi_hop_ns: timings.ffi_hop_ns,
            policy_ns: timings.policy_ns,
            verdict_egress_ns: timings.verdict_egress_ns,
        }
    }
}

// see: cans/14-observability.md#Metrics-pipeline
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtlpCausalDeltaRecord {
    pub memory_diff_bytes: u64,
    pub modified_inodes: Vec<String>,
    pub spawned_pids: Vec<u32>,
    pub egress_bytes: u64,
    pub storage_delta_root: String,
    pub receipt_leaf_hash: String,
}

// see: cans/14-observability.md#Metrics-pipeline
impl OtlpCausalDeltaRecord {
    pub fn from_delta(delta: &CausalDelta) -> Self {
        Self {
            memory_diff_bytes: delta.memory_diff_bytes,
            modified_inodes: paths_to_strings(&delta.modified_inodes),
            spawned_pids: delta.spawned_pids.clone(),
            egress_bytes: delta.egress_bytes,
            storage_delta_root: kelir_interface::hex_codec::encode_hex(&delta.storage_delta_root),
            receipt_leaf_hash: kelir_interface::hex_codec::encode_hex(&delta.receipt_leaf_hash),
        }
    }
}
