#![forbid(unsafe_code)]

#[path = "catalog_db.flow.rs"]
pub mod catalog_db;
#[path = "catalog_db.run.rs"]
pub mod catalog_db_io;
#[path = "schema_catalog.flow.rs"]
pub mod schema_catalog;

pub use catalog_db::{CatalogDb, catalog_path};

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
