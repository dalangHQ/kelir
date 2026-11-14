#![forbid(unsafe_code)]
// see: cans/05-storage-plane.md#Storage-architecture
#[path = "io.run.rs"]
mod io;
#[path = "storage_io.flow.rs"]
pub mod storage_io;

pub use crate::storage_io::storage_io;
