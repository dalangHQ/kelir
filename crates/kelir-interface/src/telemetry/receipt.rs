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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    pub correlation_token: Option<String>,
    pub dirty_bytes: u64,
    pub syscall_count: Option<u64>,
    pub exit_code: i32,
    pub wall_clock_ns: u64,
    pub phase_timings: Option<PhaseTimings>,
    pub egress_bytes: u64,
    pub egress_dropped_bytes: u64,
    pub extensions: Vec<TelemetryExtension>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestationToken {
    pub session_token: String,
    pub vmlinux_blake3: String,
    pub erofs_blake3: String,
}

// see: cans/10-attestation.md#Execution-receipts
fn hash_u64(hasher: &mut blake3::Hasher, value: u64) {
    hasher.update(&value.to_le_bytes());
}

// see: cans/10-attestation.md#Execution-receipts
fn hash_str(hasher: &mut blake3::Hasher, value: &str) {
    hash_u64(hasher, value.len() as u64);
    hasher.update(value.as_bytes());
}

// see: cans/10-attestation.md#Execution-receipts
fn hash_opt_str(hasher: &mut blake3::Hasher, value: &Option<String>) {
    match value {
        Some(text) => {
            hasher.update(&[1u8]);
            hash_str(hasher, text);
        }
        None => {
            hasher.update(&[0u8]);
        }
    }
}

// see: cans/10-attestation.md#Execution-receipts
fn hash_opt_u64(hasher: &mut blake3::Hasher, value: &Option<u64>) {
    match value {
        Some(number) => {
            hasher.update(&[1u8]);
            hash_u64(hasher, *number);
        }
        None => {
            hasher.update(&[0u8]);
        }
    }
}

// see: cans/10-attestation.md#Execution-receipts
fn hash_exit_kind(hasher: &mut blake3::Hasher, value: &ExitKind) {
    match value {
        ExitKind::Normal => {
            hasher.update(&[0u8]);
        }
        ExitKind::Signal(code) => {
            hasher.update(&[1u8]);
            hasher.update(&code.to_le_bytes());
        }
        ExitKind::KernelPanic(line) => {
            hasher.update(&[2u8]);
            hash_str(hasher, line);
        }
        ExitKind::HypervisorFault(line) => {
            hasher.update(&[3u8]);
            hash_str(hasher, line);
        }
        ExitKind::DirtyShutdown => {
            hasher.update(&[4u8]);
        }
        ExitKind::HardKilled => {
            hasher.update(&[5u8]);
        }
    }
}

impl ExecutionReceipt {
    // see: cans/10-attestation.md#Execution-receipts
    pub fn leaf_hash(&self) -> String {
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
            extensions: _,
        } = self;
        let mut hasher = blake3::Hasher::new();
        hash_str(&mut hasher, "kelir-receipt-leaf-v1");
        hash_u64(&mut hasher, u64::from(*pid));
        hash_str(&mut hasher, binary_path);
        hash_str(&mut hasher, binary_blake3);
        hash_exit_kind(&mut hasher, exit_kind);
        hash_opt_str(&mut hasher, correlation_token);
        hash_u64(&mut hasher, *dirty_bytes);
        hash_opt_u64(&mut hasher, syscall_count);
        hasher.update(&exit_code.to_le_bytes());
        hash_u64(&mut hasher, *egress_bytes);
        hash_u64(&mut hasher, *egress_dropped_bytes);
        hasher.finalize().to_hex().to_string()
    }

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
            extensions: _,
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
            extensions: _,
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
    }
}
