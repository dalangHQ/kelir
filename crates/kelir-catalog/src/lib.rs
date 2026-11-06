#![forbid(unsafe_code)]

pub mod schema;

use crate::schema::{SCHEMA_V1_SQL, SessionRow};
use thiserror::Error;

// see: cans/06-storage-catalog.md#Entity-model
#[derive(Error, Debug)]
pub enum CatalogError {
    #[error("catalog sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("catalog operation not implemented (L3): {0}")]
    NotImplemented(String),
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
        let _statement = self.pool.prepare(
            "INSERT INTO sessions (session_id, incarnation_id, lease_kind, expires_at_unix, runner_pid, boot_id, start_time_ns) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        let _ = row;
        // TODO (L3): bind the session row and execute the insert.
        Err(CatalogError::NotImplemented(
            "register_session execute (L3)".to_string(),
        ))
    }

    pub fn heartbeat_lease(
        &self,
        session_id: &[u8; 16],
        ttl_seconds: u32,
    ) -> Result<(), CatalogError> {
        let _statement = self
            .pool
            .prepare("UPDATE sessions SET expires_at_unix = ?2 WHERE session_id = ?1")?;
        let _ = (session_id, ttl_seconds);
        // TODO (L3): bind the lease expiry and execute the update.
        Err(CatalogError::NotImplemented(
            "heartbeat_lease execute (L3)".to_string(),
        ))
    }

    pub fn prune_cas_leases(&self) -> Result<u64, CatalogError> {
        let _statement = self
            .pool
            .prepare("DELETE FROM cas_leases WHERE expires_at_unix <= ?1")?;
        // TODO (L3): bind now_unix and execute the delete, returning the pruned count.
        Err(CatalogError::NotImplemented(
            "prune_cas_leases execute (L3)".to_string(),
        ))
    }
}
