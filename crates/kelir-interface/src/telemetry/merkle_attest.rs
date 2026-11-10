// see: cans/10-attestation.md#Execution-receipts
use super::{ExecutionReceipt, ExitKind, ReceiptExtension};

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

// see: cans/10-attestation.md#Execution-receipts
impl ExecutionReceipt {
    // see: cans/10-attestation.md#Execution-receipts
    pub fn leaf_hash(&self) -> [u8; 32] {
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
        hash_u64(&mut hasher, extensions.len() as u64);
        for item in extensions {
            let ReceiptExtension { namespace, payload } = item;
            hash_str(&mut hasher, namespace);
            hash_u64(&mut hasher, payload.len() as u64);
            hasher.update(payload);
        }
        *hasher.finalize().as_bytes()
    }
}
