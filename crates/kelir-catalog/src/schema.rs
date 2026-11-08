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

impl LeaseKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            LeaseKind::Ephemeral => "ephemeral",
            LeaseKind::Lease => "lease",
        }
    }
}

impl std::str::FromStr for LeaseKind {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "ephemeral" => Ok(LeaseKind::Ephemeral),
            "lease" => Ok(LeaseKind::Lease),
            _ => Err("unknown lease_kind"),
        }
    }
}

// see: cans/06-storage-catalog.md#Entity-model
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IncarnationId(pub [u8; 16]);

impl IncarnationId {
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

// see: cans/06-storage-catalog.md#Entity-model
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CasLease {
    pub blob_hash: Vec<u8>,
    pub holder_pid: i64,
    pub expires_at_unix: i64,
}

// see: cans/06-storage-catalog.md#Entity-model
#[derive(Debug, Clone)]
pub struct SessionRow {
    pub session_id: [u8; 16],
    pub incarnation_id: IncarnationId,
    pub lease_kind: LeaseKind,
    pub expires_at_unix: Option<i64>,
    pub runner_pid: i64,
    pub boot_id: String,
    pub start_time_ns: i64,
}
