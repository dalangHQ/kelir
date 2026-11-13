#![forbid(unsafe_code)]
// see: cans/05-storage-plane.md#Storage-architecture
#[path = "io.run.rs"]
pub mod io;

pub use io::RealStorageIo;
