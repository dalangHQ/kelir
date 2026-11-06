#![forbid(unsafe_code)]

pub mod schema;

use crate::schema::{SCHEMA_V1_SQL, SessionRow};

pub struct CatalogDb {
    pub pool: rusqlite::Connection,
}

impl CatalogDb {
    pub fn open<P: AsRef<std::path::Path>>(path: P) -> Result<Self, rusqlite::Error> {
        let pool = rusqlite::Connection::open(path)?;
        // TODO: pool.execute_batch(SCHEMA_V1_SQL).
        let _ = SCHEMA_V1_SQL;
        Ok(Self { pool })
    }

    pub fn register_session(&self, row: &SessionRow) -> Result<(), rusqlite::Error> {
        let _ = row;
        // TODO: INSERT session row SQL.
        Err(rusqlite::Error::InvalidQuery)
    }

    pub fn heartbeat_lease(
        &self,
        session_id: &[u8; 16],
        ttl_seconds: u32,
    ) -> Result<(), rusqlite::Error> {
        let _ = (session_id, ttl_seconds);
        // TODO: UPDATE sessions SET expires_at_unix SQL.
        Err(rusqlite::Error::InvalidQuery)
    }

    pub fn prune_cas_leases(&self) -> Result<u64, rusqlite::Error> {
        // TODO: DELETE expired cas_leases SQL.
        Err(rusqlite::Error::InvalidQuery)
    }
}
