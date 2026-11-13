#![forbid(unsafe_code)]

#[path = "schema_catalog.flow.rs"]
pub mod schema_catalog;

use crate::schema_catalog::{IncarnationId, LeaseKind, SCHEMA_SQL, SessionRow};
use rusqlite::params;
use thiserror::Error;

// see: cans/06-storage-catalog.md#Entity-model
#[derive(Error, Debug)]
pub enum CatalogError {
    #[error("catalog sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("HOME is not set; cannot locate ~/.kelir/catalog.db")]
    HomeUnset,
    #[error("catalog host state root unavailable: {0}")]
    HostRoot(#[from] std::io::Error),
}

fn now_unix() -> i64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    i64::try_from(secs).unwrap_or(i64::MAX)
}

pub struct CatalogDb {
    pub pool: rusqlite::Connection,
}

// see: cans/01-architecture.md#Host-state-root
pub fn catalog_path() -> Result<std::path::PathBuf, CatalogError> {
    let home = std::env::var("HOME").map_err(|_| CatalogError::HomeUnset)?;
    let root = std::path::PathBuf::from(home).join(".kelir");
    std::fs::create_dir_all(&root)?;
    Ok(root.join("catalog.db"))
}

impl CatalogDb {
    pub fn open<P: AsRef<std::path::Path>>(path: P) -> Result<Self, rusqlite::Error> {
        if let Some(parent) = path.as_ref().parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let pool = rusqlite::Connection::open(path)?;
        pool.execute_batch(SCHEMA_SQL)?;
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

    pub fn lookup_session(
        &self,
        session_id: &[u8; 16],
    ) -> Result<Option<SessionRow>, CatalogError> {
        let mut stmt = self.pool.prepare(
            "SELECT session_id, incarnation_id, lease_kind, expires_at_unix, runner_pid, boot_id, start_time_ns FROM sessions WHERE session_id = ?1",
        )?;
        let mut rows = stmt.query(params![&session_id[..]])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let session_bytes: Vec<u8> = row.get(0)?;
        let incarnation_bytes: Vec<u8> = row.get(1)?;
        let lease_kind_raw: String = row.get(2)?;
        let Some(lease_kind) = lease_kind_raw.parse::<LeaseKind>().ok() else {
            return Ok(None);
        };
        let Some(session_id) = <[u8; 16]>::try_from(session_bytes).ok() else {
            return Ok(None);
        };
        let Some(incarnation_id) = <[u8; 16]>::try_from(incarnation_bytes)
            .ok()
            .map(IncarnationId)
        else {
            return Ok(None);
        };
        Ok(Some(SessionRow {
            session_id,
            incarnation_id,
            lease_kind,
            expires_at_unix: row.get(3)?,
            runner_pid: row.get(4)?,
            boot_id: row.get(5)?,
            start_time_ns: row.get(6)?,
        }))
    }

    pub fn unregister_session(&self, session_id: &[u8; 16]) -> Result<bool, CatalogError> {
        let count = self.pool.execute(
            "DELETE FROM sessions WHERE session_id = ?1",
            rusqlite::params![&session_id[..]],
        )?;
        Ok(count > 0)
    }

    // see: cans/08-lifecycle.md#Detached-lifecycle
    pub fn disk_lock_handoff(
        &self,
        session_id: &[u8; 16],
        runner_pid: i64,
        ttl_seconds: u32,
    ) -> Result<bool, CatalogError> {
        // TODO (L3) [storage_plane]: no pending/owned lock-state column exists in SCHEMA_SQL or cans/06; claim is runner_pid plus lease refresh until the schema gains one.; slop: claim accepted on runner_pid alone, CAS mismatch unproven
        let now = now_unix();
        let expires = now.saturating_add(i64::from(ttl_seconds));
        let claimed = self.pool.execute(
            "UPDATE sessions SET runner_pid = ?2, expires_at_unix = ?3 WHERE session_id = ?1 AND (expires_at_unix <= ?4 OR runner_pid = ?2)",
            params![&session_id[..], runner_pid, expires, now],
        )?;
        Ok(claimed == 1)
    }

    pub fn heartbeat_lease(
        &self,
        session_id: &[u8; 16],
        runner_pid: i64,
        ttl_seconds: u32,
    ) -> Result<bool, CatalogError> {
        let expires = now_unix().saturating_add(i64::from(ttl_seconds));
        let refreshed = self.pool.execute(
            "UPDATE sessions SET expires_at_unix = ?2 WHERE session_id = ?1 AND runner_pid = ?3",
            params![&session_id[..], expires, runner_pid],
        )?;
        Ok(refreshed == 1)
    }

    pub fn prune_cas_leases(&self) -> Result<u64, CatalogError> {
        let pruned = self.pool.execute(
            "DELETE FROM cas_leases WHERE expires_at_unix <= ?1",
            params![now_unix()],
        )?;
        Ok(pruned as u64)
    }

    pub fn prune_expired_sessions(&self) -> Result<u64, CatalogError> {
        let pruned = self.pool.execute(
            "DELETE FROM sessions WHERE expires_at_unix <= ?1",
            params![now_unix()],
        )?;
        Ok(pruned as u64)
    }
}
