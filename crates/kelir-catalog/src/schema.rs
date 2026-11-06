// see: cans/06-storage-catalog.md#Entity-model

pub const SCHEMA_V1_SQL: &str = "
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY
);
CREATE TABLE IF NOT EXISTS sessions (
    session_id BLOB PRIMARY KEY,
    incarnation_id BLOB NOT NULL,
    lease_kind TEXT CHECK(lease_kind IN ('ephemeral', 'lease')) NOT NULL,
    expires_at_unix INTEGER,
    runner_pid INTEGER NOT NULL,
    boot_id TEXT NOT NULL,
    start_time_ns INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS cas_leases (
    blob_hash BLOB NOT NULL,
    holder_pid INTEGER NOT NULL,
    expires_at_unix INTEGER NOT NULL,
    PRIMARY KEY (blob_hash, holder_pid)
);
";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseKind {
    Ephemeral,
    Lease,
}

#[derive(Debug, Clone)]
pub struct SessionRow {
    pub session_id: String,
    pub status: String,
    pub incarnation_id: String,
    pub boot_id: String,
    pub start_time_ns: u64,
    pub pid: u32,
    pub lease_kind: LeaseKind,
    pub expires_at_unix: Option<u64>,
}
