// see: cans/12-adapters.md#Ingestion-matrix
#![forbid(unsafe_code)]

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
    #[error("usage: kelir-pack <oci-manifest> <erofs-output>")]
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

#[derive(Debug, Clone)]
pub struct PackArgs {
    pub manifest: PathBuf,
    pub output: PathBuf,
}

impl PackArgs {
    pub fn from_argv(argv: &[String]) -> Result<Self, PackError> {
        if argv.len() != 3 {
            return Err(PackError::Usage);
        }
        Ok(Self {
            manifest: PathBuf::from(&argv[1]),
            output: PathBuf::from(&argv[2]),
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct RadixTree {
    pub inode_count: u32,
}

impl RadixTree {
    pub fn new() -> Self {
        Self::default()
    }
}

fn fetch_manifest(args: &PackArgs) -> Result<OciManifest, PackError> {
    // TODO (L3): streaming HTTPS client pulls the OCI manifest and layer tar chunks directly.
    let _ = args;
    Err(PackError::FetchPending)
}

fn phase1_scan_headers(manifest: &OciManifest) -> Result<RadixTree, PackError> {
    // TODO (L3): stream layer tar headers only into the radix tree; apply whiteouts in place; compute topology, inode count, chunk extents.
    let _ = manifest;
    Err(PackError::Phase1Pending)
}

fn phase2_emit_erofs(args: &PackArgs, tree: &RadixTree) -> Result<ErofsImage, PackError> {
    // TODO (L3): stream layer payload bodies into CAS blobs; emit finalized EROFS data and directory blocks from the phase 1 map; compute the canonical rootfs BLAKE3 digest.
    let _ = (args, tree);
    Err(PackError::EmitPending)
}

fn run(args: PackArgs) -> Result<ErofsImage, PackError> {
    let manifest = fetch_manifest(&args)?;
    let tree = phase1_scan_headers(&manifest)?;
    phase2_emit_erofs(&args, &tree)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let argv: Vec<String> = std::env::args().collect();
    let args = PackArgs::from_argv(&argv)?;
    let _image = run(args)?;
    Ok(())
}
