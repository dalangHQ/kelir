// see: cans/06-storage-catalog.md#Entity-model
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

// see: cans/06-storage-catalog.md#Entity-model
pub const SCHEMA_VERSION: u32 = 2;

// see: cans/06-storage-catalog.md#Entity-model
pub const CAS_LEASE_TTL_SECONDS: u32 = 30;

// see: cans/06-storage-catalog.md#Entity-model
pub const DEFAULT_TENANT_ID: &str = "default";

// see: cans/06-storage-catalog.md#Entity-model
pub const SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS sessions (
    tenant_id TEXT NOT NULL,
    session_id BLOB NOT NULL,
    incarnation_id BLOB NOT NULL,
    lease_kind TEXT CHECK(lease_kind IN ('ephemeral', 'lease')) NOT NULL,
    expires_at_unix INTEGER,
    runner_pid INTEGER NOT NULL,
    boot_id TEXT NOT NULL,
    start_time_ns INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, session_id)
);
CREATE TABLE IF NOT EXISTS cas_leases (
    blob_hash BLOB NOT NULL,
    holder_pid INTEGER NOT NULL,
    expires_at_unix INTEGER NOT NULL,
    PRIMARY KEY (blob_hash, holder_pid)
);
CREATE INDEX IF NOT EXISTS cas_leases_expiry_idx ON cas_leases(expires_at_unix);
";

// see: cans/06-storage-catalog.md#Entity-model
pub const MIGRATION_V1_TO_V2_SQL: &str = "
CREATE TABLE IF NOT EXISTS sessions_v2 (
    tenant_id TEXT NOT NULL,
    session_id BLOB NOT NULL,
    incarnation_id BLOB NOT NULL,
    lease_kind TEXT CHECK(lease_kind IN ('ephemeral', 'lease')) NOT NULL,
    expires_at_unix INTEGER,
    runner_pid INTEGER NOT NULL,
    boot_id TEXT NOT NULL,
    start_time_ns INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, session_id)
);
INSERT OR IGNORE INTO sessions_v2 (tenant_id, session_id, incarnation_id, lease_kind, expires_at_unix, runner_pid, boot_id, start_time_ns) SELECT 'default', session_id, incarnation_id, lease_kind, expires_at_unix, runner_pid, boot_id, start_time_ns FROM sessions;
DROP TABLE sessions;
ALTER TABLE sessions_v2 RENAME TO sessions;
CREATE UNIQUE INDEX IF NOT EXISTS sessions_tenant_session_uidx ON sessions(tenant_id, session_id);
";

// see: cans/06-storage-catalog.md#Entity-model
pub const SQLITE_TENANT_INDEX_SQL: &str = "CREATE UNIQUE INDEX IF NOT EXISTS sessions_tenant_session_uidx ON sessions(tenant_id, session_id);";

// see: cans/06-storage-catalog.md#Entity-model
pub const POSTGRES_PARTITION_DDL: &str = "
CREATE TABLE IF NOT EXISTS sessions (
    tenant_id TEXT NOT NULL,
    session_id BYTEA NOT NULL,
    incarnation_id BYTEA NOT NULL,
    lease_kind TEXT CHECK (lease_kind IN ('ephemeral', 'lease')) NOT NULL,
    expires_at_unix BIGINT,
    runner_pid BIGINT NOT NULL,
    boot_id TEXT NOT NULL,
    start_time_ns BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, session_id)
) PARTITION BY LIST (tenant_id);
CREATE TABLE IF NOT EXISTS sessions_default PARTITION OF sessions DEFAULT;
CREATE TABLE IF NOT EXISTS cas_leases (
    blob_hash BYTEA NOT NULL,
    holder_pid BIGINT NOT NULL,
    expires_at_unix BIGINT NOT NULL,
    PRIMARY KEY (blob_hash, holder_pid)
);
CREATE INDEX IF NOT EXISTS cas_leases_expiry_idx ON cas_leases (expires_at_unix);
";

// see: cans/06-storage-catalog.md#Entity-model
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CatalogBackend {
    #[default]
    Sqlite,
    Postgres,
}

impl CatalogBackend {
    pub fn as_str(&self) -> &'static str {
        match self {
            CatalogBackend::Sqlite => "sqlite",
            CatalogBackend::Postgres => "postgres",
        }
    }
}

// see: cans/06-storage-catalog.md#Entity-model
pub fn tenant_partition_ddl(backend: CatalogBackend) -> &'static str {
    match backend {
        CatalogBackend::Postgres => POSTGRES_PARTITION_DDL,
        CatalogBackend::Sqlite => SQLITE_TENANT_INDEX_SQL,
    }
}

// see: cans/06-storage-catalog.md#Entity-model
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CatalogConfig {
    pub backend: CatalogBackend,
    pub endpoint: Option<String>,
    pub schema_tenant_partitioning: bool,
    pub fallback_sqlite_path: Option<String>,
}

impl CatalogConfig {
    pub fn sqlite(path: impl Into<String>) -> Self {
        Self {
            backend: CatalogBackend::Sqlite,
            endpoint: None,
            schema_tenant_partitioning: true,
            fallback_sqlite_path: Some(path.into()),
        }
    }

    pub fn postgres(endpoint: impl Into<String>, fallback_sqlite_path: impl Into<String>) -> Self {
        Self {
            backend: CatalogBackend::Postgres,
            endpoint: Some(endpoint.into()),
            schema_tenant_partitioning: true,
            fallback_sqlite_path: Some(fallback_sqlite_path.into()),
        }
    }
}

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

    pub fn new_v7() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = u64::from(std::process::id());
        let mut id = [0u8; 16];
        let ts_ms = elapsed.as_millis() as u64 & 0x0000_FFFF_FFFF_FFFF;
        id[0..6].copy_from_slice(&ts_ms.to_be_bytes()[2..8]);
        let rand_a = counter & 0x0FFF;
        id[6] = 0x70 | (rand_a >> 8) as u8;
        id[7] = (rand_a & 0xFF) as u8;
        let rand_b = ((u64::from(elapsed.subsec_nanos()) << 20) ^ (pid << 8) ^ counter)
            & 0x3FFF_FFFF_FFFF_FFFF;
        id[8] = 0x80 | (rand_b >> 56) as u8;
        id[9..16].copy_from_slice(&(rand_b & 0x00FF_FFFF_FFFF_FFFF).to_be_bytes()[1..8]);
        Self(id)
    }

    pub fn is_uuid_v7(&self) -> bool {
        self.0[6] >> 4 == 7 && self.0[8] >> 6 == 0b10
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
    pub tenant_id: String,
    pub session_id: [u8; 16],
    pub incarnation_id: IncarnationId,
    pub lease_kind: LeaseKind,
    pub expires_at_unix: Option<i64>,
    pub runner_pid: i64,
    pub boot_id: String,
    pub start_time_ns: i64,
}
