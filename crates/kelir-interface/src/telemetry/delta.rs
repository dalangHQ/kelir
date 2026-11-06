// crates/kelir-interface/src/telemetry/delta.rs
//
// Differential state delta between two checkpoints: Δ = S_A - S_B.
// Net logical diffs only: host CoW broker upper file + explicit
// tombstones, dirty bytes, process tree edges and exit statuses.
// No PMU samples appear in CausalDelta; PMU counters are diagnostic
// gauges only, emitted to OTel, never in receipts, CausalDelta, or
// Merkle leaves. Computes exactly what an agent branch did differently
// from its parent.
// Struct exhaustiveness tripwire: unhandled fields fail compilation in
// delta aggregation and snapshot validation.
//
// Canonical spec home: cans/10-attestation.md#Causal-delta

use std::path::PathBuf;

// Delta chain (08): one immutable Base and N stacked WorkingDeltas,
// depth unbounded. StepVerdict::Commit advances the delta DAG head;
// writes remain in sparse extents. Compaction flattens ancestor deltas
// in a single-pass block copy on export; compaction writes a new
// consolidated manifest pointing to the ancestor base. Rollback:
// the broker drops the upper file and tombstones, base untouched,
// FALLOC_FL_PUNCH_HOLE + posix_fadvise(DONTNEED); rollback also
// closes host sockets, guest sees ECONNRESET/EPIPE, sockets are not
// restored from snapshots.
// Lineage walks the CAS manifest DAG: every snapshot manifest immutably
// records its parent_digest; the catalog stores zero child arrays.
// Separately, every session registers a catalog row, no bypass:
// ephemeral lease_kind='ephemeral', expires_at NULL, reaped on parent
// death; detached lease_kind='lease', expires_at refreshed 30s; the row
// carries incarnation_id (UUIDv7 per spawn), boot_id, start_time_ns,
// pid; liveness proof Linux pidfd, macOS kqueue NOTE_EXIT, Windows Job
// Object KILL_ON_JOB_CLOSE. Lineage itself is not a field of this struct.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CausalDelta {
    /// Hardware-agnostic byte-level RAM delta
    pub memory_diff_bytes: u64,
    /// Modified inodes between the two checkpoints
    pub modified_inodes: Vec<PathBuf>,
    pub spawned_pids: Vec<u32>,
    /// Wire egress bytes between the two checkpoints
    pub egress_bytes: u64,
    /// BLAKE3 root of modified storage inodes (from StorageDelta)
    pub storage_delta_root: [u8; 32],
    /// Merkle leaf hash of the associated ExecutionReceipt
    pub receipt_leaf_hash: [u8; 32],
}
