use rusqlite::params;

use crate::CatalogError;
use crate::schema_catalog::{
    CAS_LEASE_TTL_SECONDS, CatalogBackend, CatalogConfig, IncarnationId, LeaseKind,
    MIGRATION_V1_TO_V2_SQL, SCHEMA_SQL, SQLITE_TENANT_INDEX_SQL, SessionRow,
};

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

// see: cans/06-storage-catalog.md#Entity-model
pub fn expand_catalog_path(path: &str) -> Result<std::path::PathBuf, CatalogError> {
    if let Some(rest) = path.strip_prefix("~/") {
        let home = std::env::var("HOME").map_err(|_| CatalogError::HomeUnset)?;
        return Ok(std::path::PathBuf::from(home).join(rest));
    }
    if path == "~" {
        let home = std::env::var("HOME").map_err(|_| CatalogError::HomeUnset)?;
        return Ok(std::path::PathBuf::from(home));
    }
    Ok(std::path::PathBuf::from(path))
}

// see: cans/06-storage-catalog.md#Entity-model
fn sessions_has_tenant_column(pool: &rusqlite::Connection) -> Result<bool, rusqlite::Error> {
    let mut stmt = pool.prepare("PRAGMA table_info(sessions)")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == "tenant_id" {
            return Ok(true);
        }
    }
    Ok(false)
}

impl CatalogDb {
    pub fn open<P: AsRef<std::path::Path>>(path: P) -> Result<Self, rusqlite::Error> {
        if let Some(parent) = path.as_ref().parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let pool = rusqlite::Connection::open(path)?;
        pool.execute_batch(SCHEMA_SQL)?;
        if !sessions_has_tenant_column(&pool)? {
            pool.execute_batch(MIGRATION_V1_TO_V2_SQL)?;
        }
        pool.execute_batch(SQLITE_TENANT_INDEX_SQL)?;
        pool.execute_batch("PRAGMA user_version = 2;")?;
        // see: cans/01-architecture.md#Host-state-root
        pool.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
        Ok(Self { pool })
    }

    // see: cans/06-storage-catalog.md#Entity-model
    pub fn open_with_config(config: &CatalogConfig) -> Result<Self, CatalogError> {
        match config.backend {
            CatalogBackend::Sqlite => {
                let path = match config.fallback_sqlite_path.as_deref() {
                    Some(path) => expand_catalog_path(path)?,
                    None => catalog_path()?,
                };
                Self::open(path).map_err(CatalogError::Sqlite)
            }
            CatalogBackend::Postgres => {
                let endpoint = config.endpoint.clone().unwrap_or_default();
                crate::catalog_db_io::open_postgres(&endpoint)
            }
        }
    }

    pub fn register_session(&self, row: &SessionRow) -> Result<(), CatalogError> {
        self.pool.execute(
            "INSERT INTO sessions (tenant_id, session_id, incarnation_id, lease_kind, expires_at_unix, runner_pid, boot_id, start_time_ns) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                &row.tenant_id,
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
        tenant_id: &str,
        session_id: &[u8; 16],
    ) -> Result<Option<SessionRow>, CatalogError> {
        let mut stmt = self.pool.prepare(
            "SELECT tenant_id, session_id, incarnation_id, lease_kind, expires_at_unix, runner_pid, boot_id, start_time_ns FROM sessions WHERE tenant_id = ?1 AND session_id = ?2",
        )?;
        let mut rows = stmt.query(params![tenant_id, &session_id[..]])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let tenant_id: String = row.get(0)?;
        let session_bytes: Vec<u8> = row.get(1)?;
        let incarnation_bytes: Vec<u8> = row.get(2)?;
        let lease_kind_raw: String = row.get(3)?;
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
            tenant_id,
            session_id,
            incarnation_id,
            lease_kind,
            expires_at_unix: row.get(4)?,
            runner_pid: row.get(5)?,
            boot_id: row.get(6)?,
            start_time_ns: row.get(7)?,
        }))
    }

    // see: cans/06-storage-catalog.md#Entity-model
    pub fn lookup_session_any_tenant(
        &self,
        session_id: &[u8; 16],
    ) -> Result<Option<SessionRow>, CatalogError> {
        let mut stmt = self.pool.prepare(
            "SELECT tenant_id, session_id, incarnation_id, lease_kind, expires_at_unix, runner_pid, boot_id, start_time_ns FROM sessions WHERE session_id = ?1 ORDER BY tenant_id ASC LIMIT 1",
        )?;
        let mut rows = stmt.query(params![&session_id[..]])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let tenant_id: String = row.get(0)?;
        let session_bytes: Vec<u8> = row.get(1)?;
        let incarnation_bytes: Vec<u8> = row.get(2)?;
        let lease_kind_raw: String = row.get(3)?;
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
            tenant_id,
            session_id,
            incarnation_id,
            lease_kind,
            expires_at_unix: row.get(4)?,
            runner_pid: row.get(5)?,
            boot_id: row.get(6)?,
            start_time_ns: row.get(7)?,
        }))
    }

    pub fn unregister_session(
        &self,
        tenant_id: &str,
        session_id: &[u8; 16],
    ) -> Result<bool, CatalogError> {
        let count = self.pool.execute(
            "DELETE FROM sessions WHERE tenant_id = ?1 AND session_id = ?2",
            rusqlite::params![tenant_id, &session_id[..]],
        )?;
        Ok(count > 0)
    }

    // see: cans/08-lifecycle.md#Detached-lifecycle
    pub fn disk_lock_handoff(
        &self,
        tenant_id: &str,
        session_id: &[u8; 16],
        runner_pid: i64,
        ttl_seconds: u32,
    ) -> Result<bool, CatalogError> {
        // L3-GAP: [storage_plane]: no pending/owned lock-state column exists in SCHEMA_SQL or cans/06; claim is runner_pid plus lease refresh until the schema gains one.; slop: claim accepted on runner_pid alone, CAS mismatch unproven
        let now = now_unix();
        let expires = now.saturating_add(i64::from(ttl_seconds));
        let claimed = self.pool.execute(
            "UPDATE sessions SET runner_pid = ?3, expires_at_unix = ?4 WHERE tenant_id = ?1 AND session_id = ?2 AND (expires_at_unix <= ?5 OR runner_pid = ?3)",
            params![tenant_id, &session_id[..], runner_pid, expires, now],
        )?;
        Ok(claimed == 1)
    }

    pub fn heartbeat_lease(
        &self,
        tenant_id: &str,
        session_id: &[u8; 16],
        runner_pid: i64,
        ttl_seconds: u32,
    ) -> Result<bool, CatalogError> {
        let expires = now_unix().saturating_add(i64::from(ttl_seconds));
        let refreshed = self.pool.execute(
            "UPDATE sessions SET expires_at_unix = ?3 WHERE tenant_id = ?1 AND session_id = ?2 AND runner_pid = ?4",
            params![tenant_id, &session_id[..], expires, runner_pid],
        )?;
        Ok(refreshed == 1)
    }

    // see: cans/06-storage-catalog.md#Entity-model
    pub fn register_cas_lease(
        &self,
        blob_hash: &[u8],
        holder_pid: i64,
    ) -> Result<i64, CatalogError> {
        let expires = now_unix().saturating_add(i64::from(CAS_LEASE_TTL_SECONDS));
        self.pool.execute(
            "INSERT INTO cas_leases (blob_hash, holder_pid, expires_at_unix) VALUES (?1, ?2, ?3)",
            params![blob_hash, holder_pid, expires],
        )?;
        Ok(expires)
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
