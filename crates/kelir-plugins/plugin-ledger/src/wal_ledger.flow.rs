// see: cans/plugins/ledger.md#Merkle-WAL-specification
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use kelir_interface::plugin_model::PluginError;
use kelir_interface::telemetry::{CausalDelta, HookSpan};
use kelir_otlp::OtlpExporter;
use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::OtelConfig;

pub trait LedgerSink: Send + Sync {
    fn emit_delta(&self, delta: &CausalDelta) -> Result<(), PluginError>;
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.hashing_standard
pub const WAL_RECORD_LEN: usize = 8 + 32 + 32 + 32 + 8 + 64;

// see: cans/plugins/ledger.md#Merkle-WAL-specification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalRecord {
    pub seq: u64,
    #[serde(with = "kelir_interface::hex_codec::serde_hex_32")]
    pub prev_hash: [u8; 32],
    #[serde(with = "kelir_interface::hex_codec::serde_hex_32")]
    pub receipt_blake3: [u8; 32],
    #[serde(with = "kelir_interface::hex_codec::serde_hex_32")]
    pub delta_root: [u8; 32],
    pub timestamp_ns: u64,
    #[serde(with = "kelir_interface::hex_codec::serde_hex_64")]
    pub signature: [u8; 64],
}

// see: cans/plugins/ledger.md#Merkle-WAL-specification
#[path = "wal_ledger/merkle_wal.flow.rs"]
pub mod merkle_wal;

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
            // L3-GAP: [telemetry_audit]: sign each leaf with the host session Ed25519 key per cans/plugins/ledger.md#Merkle-WAL-specification; slop: leaves ship with zeroed signatures, Merkle chain unverifiable
            signature: [0u8; 64],
        }
    }

    // see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.hashing_standard
    pub fn dual_leaf_hash(&self) -> DualHash {
        dual_merkle_leaf(&self.pack())
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

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.hashing_standard
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DualHash {
    pub blake3: [u8; 32],
    pub sha256: [u8; 32],
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.hashing_standard
pub fn dual_merkle_leaf(record_bytes: &[u8]) -> DualHash {
    let mut blake3_hasher = blake3::Hasher::new();
    blake3_hasher.update(&[0x00]);
    blake3_hasher.update(record_bytes);
    let mut sha256_hasher = sha2::Sha256::new();
    sha256_hasher.update([0x00]);
    sha256_hasher.update(record_bytes);
    DualHash {
        blake3: *blake3_hasher.finalize().as_bytes(),
        sha256: sha256_hasher.finalize().into(),
    }
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.hashing_standard
pub fn dual_merkle_node(left: &DualHash, right: &DualHash) -> DualHash {
    let mut blake3_hasher = blake3::Hasher::new();
    blake3_hasher.update(&[0x01]);
    blake3_hasher.update(&left.blake3);
    blake3_hasher.update(&right.blake3);
    let mut sha256_hasher = sha2::Sha256::new();
    sha256_hasher.update([0x01]);
    sha256_hasher.update(left.sha256);
    sha256_hasher.update(right.sha256);
    DualHash {
        blake3: *blake3_hasher.finalize().as_bytes(),
        sha256: sha256_hasher.finalize().into(),
    }
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.hashing_standard
pub fn dual_merkle_root(leaves: &[DualHash]) -> DualHash {
    match leaves {
        [] => DualHash {
            blake3: *blake3::hash(&[]).as_bytes(),
            sha256: sha2::Sha256::digest([]).into(),
        },
        [leaf] => *leaf,
        _ => {
            let mut split = 1usize;
            while split * 2 < leaves.len() {
                split *= 2;
            }
            dual_merkle_node(
                &dual_merkle_root(&leaves[..split]),
                &dual_merkle_root(&leaves[split..]),
            )
        }
    }
}

// see: cans/plugins/ledger.md#Merkle-WAL-specification
#[derive(Debug, Default)]
pub struct MerkleChain {
    leaves: Mutex<Vec<[u8; 32]>>,
    spans: Mutex<Vec<HookSpan>>,
}

impl MerkleChain {
    pub fn new() -> Self {
        Self::default()
    }

    // see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.record_pre_redaction_hash
    pub fn record_pre_redaction_hash(&self, record_bytes: &[u8]) -> Result<DualHash, PluginError> {
        let leaf = dual_merkle_leaf(record_bytes);
        let mut leaves = self
            .leaves
            .lock()
            .map_err(|_| PluginError::Halt("merkle chain leaf table poisoned".into()))?;
        leaves.push(leaf.blake3);
        Ok(leaf)
    }

    // see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.include_hook_spans_in_root
    pub fn record_hook_span(&self, span: HookSpan) -> Result<(), PluginError> {
        let mut spans = self
            .spans
            .lock()
            .map_err(|_| PluginError::Halt("merkle chain span table poisoned".into()))?;
        spans.push(span);
        Ok(())
    }

    // see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.include_hook_spans_in_root
    pub fn root(&self, include_hook_spans: bool) -> Result<[u8; 32], PluginError> {
        let mut leaves = self
            .leaves
            .lock()
            .map_err(|_| PluginError::Halt("merkle chain leaf table poisoned".into()))?
            .clone();
        if include_hook_spans {
            let spans = self
                .spans
                .lock()
                .map_err(|_| PluginError::Halt("merkle chain span table poisoned".into()))?;
            for span in spans.iter() {
                leaves.push(merkle_leaf(&canonical_hook_span(span)));
            }
        }
        Ok(merkle_root(&leaves))
    }
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.include_hook_spans_in_root
fn canonical_hook_span(span: &HookSpan) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(span.plugin.len() as u64).to_le_bytes());
    bytes.extend_from_slice(span.plugin.as_bytes());
    bytes.extend_from_slice(format!("{:?}", span.ring).as_bytes());
    bytes.extend_from_slice(format!("{:?}", span.verdict_kind).as_bytes());
    bytes.extend_from_slice(&span.duration_ns.to_le_bytes());
    bytes.extend_from_slice(&span.ordinal.to_le_bytes());
    bytes
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.witness_cosigning_endpoint
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedTreeHead {
    pub tree_size: u64,
    pub root: [u8; 32],
    pub timestamp_ns: u64,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.witness_cosigning_endpoint
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitnessCosignature {
    pub witness: String,
    pub signature: [u8; 64],
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.witness_cosigning_endpoint
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitnessClient {
    pub endpoint: String,
}

impl WitnessClient {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
        }
    }

    // see: cans/plugins/ledger.md#Merkle-WAL-specification
    pub async fn cosign(&self, head: &SignedTreeHead) -> Result<WitnessCosignature, PluginError> {
        if self.endpoint.is_empty() {
            return Err(PluginError::Halt(
                "witness cosigning endpoint is not configured".into(),
            ));
        }
        let _ = head;
        // TODO (L3) [telemetry_audit]: POST the signed tree head to the witness endpoint and verify the returned cosignature; slop: witness never contacted, STH unverified.
        Err(PluginError::Halt(
            "witness cosigning transport pending (L3)".into(),
        ))
    }
}

// see: cans/10-attestation.md#Hook-trace
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerkleInclusionProof {
    pub leaf_index: u64,
    pub leaf_count: u64,
    #[serde(with = "kelir_interface::hex_codec::serde_hex_32")]
    pub leaf_hash: [u8; 32],
    pub siblings: Vec<String>,
    #[serde(with = "kelir_interface::hex_codec::serde_hex_32")]
    pub root: [u8; 32],
}

impl MerkleInclusionProof {
    // see: cans/10-attestation.md#Hook-trace
    pub fn verify(&self) -> bool {
        let mut current = self.leaf_hash;
        let mut index = self.leaf_index;
        let mut level_len = self.leaf_count;
        let mut siblings = self.siblings.iter();
        while level_len > 1 {
            let Some(sibling_hex) = siblings.next() else {
                return false;
            };
            let Ok(sibling) = kelir_interface::hex_codec::decode_hex_fixed::<32>(sibling_hex)
            else {
                return false;
            };
            let mut split = 1u64;
            while split * 2 < level_len {
                split *= 2;
            }
            if index < split {
                current = merkle_node(&current, &sibling);
                level_len = split;
            } else {
                current = merkle_node(&sibling, &current);
                index -= split;
                level_len -= split;
            }
        }
        siblings.next().is_none() && current == self.root
    }
}

// see: cans/10-attestation.md#Hook-trace
pub fn inclusion_proof(leaves: &[[u8; 32]], leaf_index: usize) -> Option<MerkleInclusionProof> {
    if leaves.is_empty() || leaf_index >= leaves.len() {
        return None;
    }
    let (root, siblings) = inclusion_proof_for(leaves, leaf_index);
    Some(MerkleInclusionProof {
        leaf_index: leaf_index as u64,
        leaf_count: leaves.len() as u64,
        leaf_hash: leaves[leaf_index],
        siblings: siblings
            .iter()
            .map(|hash| kelir_interface::hex_codec::encode_hex(hash))
            .collect(),
        root,
    })
}

// see: cans/10-attestation.md#Hook-trace
fn inclusion_proof_for(leaves: &[[u8; 32]], leaf_index: usize) -> ([u8; 32], Vec<[u8; 32]>) {
    match leaves {
        [leaf] => (*leaf, Vec::new()),
        _ => {
            let mut split = 1usize;
            while split * 2 < leaves.len() {
                split *= 2;
            }
            if leaf_index < split {
                let (left_root, mut siblings) = inclusion_proof_for(&leaves[..split], leaf_index);
                let right_root = merkle_root(&leaves[split..]);
                siblings.push(right_root);
                (merkle_node(&left_root, &right_root), siblings)
            } else {
                let (right_root, mut siblings) =
                    inclusion_proof_for(&leaves[split..], leaf_index - split);
                let left_root = merkle_root(&leaves[..split]);
                siblings.push(left_root);
                (merkle_node(&left_root, &right_root), siblings)
            }
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
        // L3-GAP: [ta_09_otlp_export_failure_non_blocking]: retry export failures locally without blocking the pipeline per cans/plugins/ledger.md#OpenTelemetry; slop: export error propagates and blocks pipeline, downed collector stalls guest
        // see: cans/plugins/ledger.md#Invariants-and-policy-(ledger)
        let exporter = match self.config.exporter_config() {
            Ok(config) => OtlpExporter::with_config(config),
            Err(err) => {
                tracing::warn!("OTLP export skipped during scaffolding: {err}");
                return Ok(());
            }
        };
        if let Err(err) = exporter.export_causal_delta(delta) {
            tracing::warn!("OTLP export skipped during scaffolding: {err}");
            return Ok(());
        }
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
            // L3-GAP: [telemetry_audit]: restore the chain head and seq from the LMDB store on open per cans/plugins/ledger.md#Merkle-WAL-specification; slop: head restarts at zero on open, seq reused, chain forks
            head: Mutex::new(([0u8; 32], 0)),
        }
    }

    pub fn store_path(&self) -> &Path {
        &self.store
    }

    // see: cans/plugins/ledger.md#Merkle-WAL-specification
    fn record_for(&self, delta: &CausalDelta, timestamp_ns: u64) -> Result<WalRecord, PluginError> {
        let head = self
            .head
            .lock()
            .map_err(|_| PluginError::Halt("wal ledger head lock poisoned".into()))?;
        Ok(WalRecord::from_delta(head.1, head.0, delta, timestamp_ns))
    }
}

impl LedgerSink for LocalMerkleDbSink {
    fn emit_delta(&self, delta: &CausalDelta) -> Result<(), PluginError> {
        let record = self.record_for(delta, host_timestamp_ns())?;
        // L3-GAP: [ta_03_merkle_wal_survives_guest_rollback]: append record.pack() to the LMDB WAL, fsync per reap, sign the leaf, then advance the chain head to record.leaf_hash(); slop: no WAL append, rollback constancy passes vacuously on empty store
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
