#![forbid(unsafe_code)]

pub mod schema;

use crate::schema::{SCHEMA_V1_SQL, SessionRow};
use rusqlite::params;
use thiserror::Error;

// see: cans/06-storage-catalog.md#Entity-model
#[derive(Error, Debug)]
pub enum CatalogError {
    #[error("catalog sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

fn now_unix() -> i64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    match i64::try_from(secs) {
        Ok(v) => v,
        Err(_) => i64::MAX,
    }
}

pub struct CatalogDb {
    pub pool: rusqlite::Connection,
}

impl CatalogDb {
    pub fn open<P: AsRef<std::path::Path>>(path: P) -> Result<Self, rusqlite::Error> {
        let pool = rusqlite::Connection::open(path)?;
        pool.execute_batch(SCHEMA_V1_SQL)?;
        // see: cans/01-architecture.md#Host-state-root
        pool.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
        Ok(Self { pool })
    }

    pub fn register_session(&self, row: &SessionRow) -> Result<(), CatalogError> {
        self.pool.execute(
            "INSERT INTO sessions (session_id, incarnation_id, lease_kind, expires_at_unix, runner_pid, boot_id, start_time_ns) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                &row.session_id[..],
                &row.incarnation_id.0[..],
                row.lease_kind.as_str(),
                row.expires_at_unix,
                row.runner_pid,
                &row.boot_id,
                row.start_time_ns,
            ],
        )?;
        Ok(())
    }

    pub fn heartbeat_lease(
        &self,
        session_id: &[u8; 16],
        ttl_seconds: u32,
    ) -> Result<(), CatalogError> {
        let expires = now_unix().saturating_add(i64::from(ttl_seconds));
        self.pool.execute(
            "UPDATE sessions SET expires_at_unix = ?2 WHERE session_id = ?1",
            params![&session_id[..], expires],
        )?;
        Ok(())
    }

    pub fn prune_cas_leases(&self) -> Result<u64, CatalogError> {
        let pruned = self.pool.execute(
            "DELETE FROM cas_leases WHERE expires_at_unix <= ?1",
            params![now_unix()],
        )?;
        Ok(pruned as u64)
    }
}
