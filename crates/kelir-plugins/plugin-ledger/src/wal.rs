// see: cans/plugins/ledger.md#Merkle-WAL-specification
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use kelir_interface::plugin::PluginError;
use kelir_interface::telemetry::CausalDelta;
use kelir_otlp::OtlpExporter;

use crate::OtelConfig;

pub trait LedgerSink: Send + Sync {
    fn emit_delta(&self, delta: &CausalDelta) -> Result<(), PluginError>;
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.hashing_standard
pub const WAL_RECORD_LEN: usize = 8 + 32 + 32 + 32 + 8 + 64;

// see: cans/plugins/ledger.md#Merkle-WAL-specification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalRecord {
    pub seq: u64,
    pub prev_hash: [u8; 32],
    pub receipt_blake3: [u8; 32],
    pub delta_root: [u8; 32],
    pub timestamp_ns: u64,
    pub signature: [u8; 64],
}

// see: cans/plugins/ledger.md#Merkle-WAL-specification
pub mod merkle;

impl WalRecord {
    // see: cans/plugins/ledger.md#Merkle-WAL-specification
    pub fn from_delta(
        seq: u64,
        prev_hash: [u8; 32],
        delta: &CausalDelta,
        timestamp_ns: u64,
    ) -> Self {
        Self {
            seq,
            prev_hash,
            receipt_blake3: delta.receipt_leaf_hash,
            delta_root: delta.storage_delta_root,
            timestamp_ns,
            // TODO (L3): sign each leaf with the host session Ed25519 key per cans/plugins/ledger.md#Merkle-WAL-specification.
            signature: [0u8; 64],
        }
    }
}

// see: cans/plugins/ledger.md#Merkle-WAL-specification
pub fn merkle_leaf(record_bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[0x00]);
    hasher.update(record_bytes);
    *hasher.finalize().as_bytes()
}

// see: cans/plugins/ledger.md#Merkle-WAL-specification
pub fn merkle_node(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[0x01]);
    hasher.update(left);
    hasher.update(right);
    *hasher.finalize().as_bytes()
}

// see: cans/plugins/ledger.md#Merkle-WAL-specification
pub fn merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    match leaves {
        [] => *blake3::hash(&[]).as_bytes(),
        [leaf] => *leaf,
        _ => {
            let mut split = 1usize;
            while split * 2 < leaves.len() {
                split *= 2;
            }
            merkle_node(
                &merkle_root(&leaves[..split]),
                &merkle_root(&leaves[split..]),
            )
        }
    }
}

// see: cans/plugins/ledger.md#OpenTelemetry
#[derive(Debug, Clone)]
pub struct OtlpLedgerSink {
    config: OtelConfig,
}

impl OtlpLedgerSink {
    pub fn new(config: OtelConfig) -> Self {
        Self { config }
    }
}

impl LedgerSink for OtlpLedgerSink {
    fn emit_delta(&self, delta: &CausalDelta) -> Result<(), PluginError> {
        // TODO (L3): retry export failures locally without blocking the pipeline per cans/plugins/ledger.md#OpenTelemetry.
        let exporter = OtlpExporter::with_config(
            self.config
                .exporter_config()
                .map_err(|e| PluginError::Halt(format!("ledger OTLP sink config invalid: {e}")))?,
        );
        exporter
            .export_causal_delta(delta)
            .map_err(|e| PluginError::Halt(format!("ledger OTLP delta export failed: {e}")))?;
        Ok(())
    }
}

// see: cans/plugins/ledger.md#Invariants-and-policy-(ledger)
#[derive(Debug)]
pub struct LocalMerkleDbSink {
    store: PathBuf,
    head: Mutex<([u8; 32], u64)>,
}

impl LocalMerkleDbSink {
    pub fn new(store: impl Into<PathBuf>) -> Self {
        Self {
            store: store.into(),
            // TODO (L3): restore the chain head and seq from the LMDB store on open per cans/plugins/ledger.md#Merkle-WAL-specification.
            head: Mutex::new(([0u8; 32], 0)),
        }
    }

    pub fn store_path(&self) -> &Path {
        &self.store
    }

    // see: cans/plugins/ledger.md#Merkle-WAL-specification
    fn record_for(&self, delta: &CausalDelta, timestamp_ns: u64) -> Result<WalRecord, PluginError> {
        let head = self.head.lock().unwrap_or_else(|e| e.into_inner());
        Ok(WalRecord::from_delta(head.1, head.0, delta, timestamp_ns))
    }
}

impl LedgerSink for LocalMerkleDbSink {
    fn emit_delta(&self, delta: &CausalDelta) -> Result<(), PluginError> {
        let record = self.record_for(delta, host_timestamp_ns())?;
        // TODO (L3): append record.pack() to the LMDB WAL, fsync per reap, sign the leaf, then advance the chain head to record.leaf_hash().
        let _ = record;
        Err(PluginError::Halt(
            "LocalMerkleDbSink LMDB append (L3)".into(),
        ))
    }
}

// see: cans/plugins/ledger.md#Merkle-WAL-specification
fn host_timestamp_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_nanos() as u64)
        .unwrap_or(0)
}
