// see: cans/12-adapters.md#Ingestion-matrix
#![forbid(unsafe_code)]

#[path = "erofs.run.rs"]
pub mod erofs;
#[path = "oci.run.rs"]
pub mod oci;
#[path = "pack.flow.rs"]
mod pack;

use std::path::PathBuf;

use thiserror::Error;

pub const MAX_INODES: u32 = 500_000;
pub const MAX_EXPANSION_RATIO: u32 = 15;
pub const MAX_LAYER_BYTES: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum PackError {
    #[error("OCI manifest fetch pending (L3)")]
    FetchPending,
    #[error("phase 1 tar header scan pending (L3)")]
    Phase1Pending,
    #[error("phase 2 payload stream pending (L3)")]
    Phase2Pending,
    #[error("EROFS emit pending (L3)")]
    EmitPending,
    #[error("layer oversize: {0}")]
    LayerOversize(String),
    #[error(
        "usage: kelir-pack <oci-manifest> <erofs-output> | kelir-pack --registry <url> --namespace <ns> --tag <tag> [--auth <token>] <erofs-output>"
    )]
    Usage,
}

#[derive(Debug, Clone)]
pub struct OciLayer {
    pub digest: String,
    pub media_type: String,
    pub size: u64,
}

#[derive(Debug, Clone, Default)]
pub struct OciManifest {
    pub layers: Vec<OciLayer>,
}

#[derive(Debug, Clone)]
pub struct ErofsImage {
    pub path: PathBuf,
    pub rootfs_digest: String,
}

#[derive(Debug, Clone, Default)]
pub struct PackArgs {
    pub manifest: Option<PathBuf>,
    pub output: PathBuf,
    pub registry: Option<String>,
    pub namespace: Option<String>,
    pub tag: Option<String>,
    pub auth: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RadixTree {
    pub inode_count: u32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let argv: Vec<String> = std::env::args().collect();
    let args = PackArgs::from_argv(&argv)?;
    let _image = pack::run(args)?;
    Ok(())
}
