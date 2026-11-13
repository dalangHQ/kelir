// see: cans/plugins/ledger.md#Merkle-WAL-specification
// TODO (L3) [telemetry_audit]: LMDB WAL append + Ed25519 leaf signing for LocalMerkleDbSink::emit_delta and LedgerCtx::finalize (restore the chain head and seq on open, sign each leaf with the host session key); slop: no WAL append, leaves ship with zeroed signatures, finalize keeps Err Halt
// TODO (L3) [ta_03_merkle_wal_survives_guest_rollback]: append record.pack() to the LMDB WAL, fsync per reap, sign the leaf, then advance the chain head to record.leaf_hash(); slop: no WAL append, rollback constancy passes vacuously on empty store
