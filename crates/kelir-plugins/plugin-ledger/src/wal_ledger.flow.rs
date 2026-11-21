// see: cans/plugins/ledger.md#Merkle-WAL-specification
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::otlp_exporter::OtlpExporter;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::telemetry::{CausalDelta, ExecutionReceipt, HookSpan};
use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::OtelConfig;
use crate::compute_ledger::StorageDelta;

pub trait LedgerSink: Send + Sync {
    fn emit_delta(&self, delta: &CausalDelta) -> Result<(), PluginError>;
}

// see: cans/10-attestation.md#Causal-delta
pub fn compute_causal_delta(
    storage: &StorageDelta,
    receipt: &ExecutionReceipt,
) -> Result<CausalDelta, PluginError> {
    Ok(CausalDelta::new(
        storage.memory_diff_bytes,
        storage.modified_inodes.clone(),
        storage.spawned_pids.clone(),
        storage.egress_bytes,
        storage.storage_delta_root,
        receipt.leaf_hash(),
    ))
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.hashing_standard
pub const WAL_RECORD_LEN: usize = 8 + 32 + 32 + 32 + 8 + 64;

// see: cans/plugins/ledger.md#Strict-audit-and-drop-accounting
pub const WAL_SEGMENT_HEADER_LEN: usize = 32;

// see: cans/plugins/ledger.md#Strict-audit-and-drop-accounting
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AuditDropCounters {
    pub overflow: u64,
    pub crc_corruption: u64,
    pub mapper: u64,
    pub auth_loss: u64,
}

// see: cans/plugins/ledger.md#Strict-audit-and-drop-accounting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditDropClass {
    Overflow,
    CrcCorruption,
    Mapper,
    AuthLoss,
}

impl AuditDropCounters {
    // see: cans/plugins/ledger.md#Strict-audit-and-drop-accounting
    pub fn record(&mut self, class: AuditDropClass) {
        match class {
            AuditDropClass::Overflow => self.overflow += 1,
            AuditDropClass::CrcCorruption => self.crc_corruption += 1,
            AuditDropClass::Mapper => self.mapper += 1,
            AuditDropClass::AuthLoss => self.auth_loss += 1,
        }
    }

    // see: cans/plugins/ledger.md#Strict-audit-and-drop-accounting
    pub fn total(&self) -> u64 {
        self.overflow + self.crc_corruption + self.mapper + self.auth_loss
    }
}

// see: cans/plugins/ledger.md#Strict-audit-and-drop-accounting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OcsfClassId(pub u16);

impl OcsfClassId {
    // see: cans/plugins/ledger.md#Strict-audit-and-drop-accounting
    pub const SECURITY_FINDING: Self = Self(1001);
    // see: cans/plugins/ledger.md#Strict-audit-and-drop-accounting
    pub const AUTHENTICATION: Self = Self(3002);
}

// see: cans/14-observability.md#Security-events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditEventKind {
    HostContainmentBreach,
    EmergencyKillSwitch,
    BreakGlass,
    ApprovalHold,
    AuditDrop,
    KeyFingerprintMismatch,
    WitnessCosignFailure,
}

// see: cans/plugins/ledger.md#Strict-audit-and-drop-accounting
pub fn ocsf_class_id(kind: AuditEventKind) -> OcsfClassId {
    match kind {
        AuditEventKind::HostContainmentBreach => OcsfClassId::SECURITY_FINDING,
        AuditEventKind::EmergencyKillSwitch => OcsfClassId::SECURITY_FINDING,
        AuditEventKind::BreakGlass => OcsfClassId::AUTHENTICATION,
        AuditEventKind::ApprovalHold => OcsfClassId::AUTHENTICATION,
        AuditEventKind::AuditDrop => OcsfClassId::SECURITY_FINDING,
        AuditEventKind::KeyFingerprintMismatch => OcsfClassId::SECURITY_FINDING,
        AuditEventKind::WitnessCosignFailure => OcsfClassId::SECURITY_FINDING,
    }
}

// see: cans/plugins/ledger.md#Merkle-WAL-specification
const fn crc32c_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut index = 0usize;
    while index < 256 {
        let mut crc = index as u32;
        let mut bit = 0u32;
        while bit < 8 {
            crc = if crc & 1 == 1 {
                0x82F6_3B78 ^ (crc >> 1)
            } else {
                crc >> 1
            };
            bit += 1;
        }
        table[index] = crc;
        index += 1;
    }
    table
}

// see: cans/plugins/ledger.md#Merkle-WAL-specification
const CRC32C_TABLE: [u32; 256] = crc32c_table();

// see: cans/plugins/ledger.md#Merkle-WAL-specification
pub const fn crc32c(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    let mut index = 0usize;
    while index < data.len() {
        let byte = data[index];
        crc = CRC32C_TABLE[((crc ^ (byte as u32)) & 0xFF) as usize] ^ (crc >> 8);
        index += 1;
    }
    crc ^ 0xFFFF_FFFFu32
}

// see: cans/plugins/ledger.md#Merkle-WAL-specification
const _: () = assert!(crc32c(b"123456789") == 0xE306_9283);

// see: cans/plugins/ledger.md#Merkle-WAL-specification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalSegmentHeader {
    pub version: u16,
    pub segment_seq: u64,
    pub record_count: u32,
    pub ack_high_watermark: u64,
}

impl WalSegmentHeader {
    // see: cans/plugins/ledger.md#Merkle-WAL-specification
    pub fn pack(&self) -> [u8; WAL_SEGMENT_HEADER_LEN] {
        let mut bytes = [0u8; WAL_SEGMENT_HEADER_LEN];
        bytes[0..4].copy_from_slice(b"KELW");
        bytes[4..6].copy_from_slice(&self.version.to_le_bytes());
        bytes[6..8].copy_from_slice(&0u16.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.segment_seq.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.record_count.to_le_bytes());
        bytes[20..28].copy_from_slice(&self.ack_high_watermark.to_le_bytes());
        let crc = crc32c(&bytes[..28]);
        bytes[28..32].copy_from_slice(&crc.to_le_bytes());
        bytes
    }

    // see: cans/plugins/ledger.md#Merkle-WAL-specification
    pub fn unpack(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != WAL_SEGMENT_HEADER_LEN || &bytes[0..4] != b"KELW" {
            return None;
        }
        let crc = u32::from_le_bytes([bytes[28], bytes[29], bytes[30], bytes[31]]);
        if crc32c(&bytes[..28]) != crc {
            return None;
        }
        Some(Self {
            version: u16::from_le_bytes([bytes[4], bytes[5]]),
            segment_seq: u64::from_le_bytes([
                bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14],
                bytes[15],
            ]),
            record_count: u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]),
            ack_high_watermark: u64::from_le_bytes([
                bytes[20], bytes[21], bytes[22], bytes[23], bytes[24], bytes[25], bytes[26],
                bytes[27],
            ]),
        })
    }
}

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
            // TODO (L3) [telemetry_audit]: sign each leaf with the host session Ed25519 key per cans/plugins/ledger.md#Merkle-WAL-specification; slop: leaves ship with zeroed signatures, Merkle chain unverifiable
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
        // TODO (L3) [ta_09_otlp_export_failure_non_blocking]: retry export failures locally without blocking the pipeline per cans/plugins/ledger.md#OpenTelemetry; slop: export error propagates and blocks pipeline, downed collector stalls guest
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
    // see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.store_path
    store_path: PathBuf,
    head: Mutex<([u8; 32], u64)>,
}

impl LocalMerkleDbSink {
    pub fn new(store_path: impl Into<PathBuf>) -> Self {
        Self {
            store_path: store_path.into(),
            // TODO (L3) [telemetry_audit]: restore the chain head and seq from the LMDB store on open per cans/plugins/ledger.md#Merkle-WAL-specification; slop: head restarts at zero on open, seq reused, chain forks
            head: Mutex::new(([0u8; 32], 0)),
        }
    }

    pub fn store_path(&self) -> &Path {
        &self.store_path
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
        // TODO (L3) [ta_03_merkle_wal_survives_guest_rollback]: append record.pack() to the LMDB WAL, fsync per reap, sign the leaf, then advance the chain head to record.leaf_hash(); slop: no WAL append, rollback constancy passes vacuously on empty store
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
