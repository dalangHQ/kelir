// see: cans/06-storage-catalog.md#Entity-model
#![forbid(unsafe_code)]

#[path = "catalog_db.flow.rs"]
pub mod catalog_db;
#[path = "catalog_db.run.rs"]
pub mod catalog_db_io;
#[path = "schema_catalog.flow.rs"]
pub mod schema_catalog;

pub use catalog_db::{CatalogDb, catalog_path, expand_catalog_path};
pub use schema_catalog::{
    CAS_LEASE_TTL_SECONDS, CatalogBackend, CatalogConfig, DEFAULT_TENANT_ID,
    MIGRATION_V1_TO_V2_SQL, POSTGRES_PARTITION_DDL, SCHEMA_SQL, SCHEMA_VERSION,
    SQLITE_TENANT_INDEX_SQL, tenant_partition_ddl,
};

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
    #[error("postgres catalog boot requires a live PostgreSQL endpoint (L3)")]
    PostgresLiveBoot,
}
