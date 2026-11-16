// see: cans/06-storage-catalog.md#Entity-model
// L3-GAP: [storage_plane]: live PostgreSQL catalog boot needs a postgres client session against the configured endpoint; the SQLite fallback is the offline path until then.; slop: postgres backend accepted at config time, boot against a dead endpoint unproven
use crate::{CatalogDb, CatalogError};

// see: cans/06-storage-catalog.md#Entity-model
pub fn open_postgres(_endpoint: &str) -> Result<CatalogDb, CatalogError> {
    // TODO (L3) [storage_plane]: open a live PostgreSQL session and apply POSTGRES_PARTITION_DDL.; slop: postgres backend accepted at config time, boot against a dead endpoint unproven
    Err(CatalogError::PostgresLiveBoot)
}
