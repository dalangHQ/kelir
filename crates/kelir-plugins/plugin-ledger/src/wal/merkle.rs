// see: cans/plugins/ledger.md#Merkle-WAL-specification
use super::{WAL_RECORD_LEN, WalRecord, merkle_leaf};

// see: cans/plugins/ledger.md#Merkle-WAL-specification
const _: () = assert!(WAL_RECORD_LEN == 176);

// see: cans/plugins/ledger.md#Merkle-WAL-specification
impl WalRecord {
    // see: cans/plugins/ledger.md#Merkle-WAL-specification
    pub fn pack(&self) -> [u8; WAL_RECORD_LEN] {
        let Self {
            seq,
            prev_hash,
            receipt_blake3,
            delta_root,
            timestamp_ns,
            signature,
        } = self;
        let mut bytes = [0u8; WAL_RECORD_LEN];
        bytes[0..8].copy_from_slice(&seq.to_le_bytes());
        bytes[8..40].copy_from_slice(prev_hash);
        bytes[40..72].copy_from_slice(receipt_blake3);
        bytes[72..104].copy_from_slice(delta_root);
        bytes[104..112].copy_from_slice(&timestamp_ns.to_le_bytes());
        bytes[112..176].copy_from_slice(signature);
        bytes
    }
    // see: cans/plugins/ledger.md#Merkle-WAL-specification
    pub fn leaf_hash(&self) -> [u8; 32] {
        merkle_leaf(&self.pack())
    }
}
