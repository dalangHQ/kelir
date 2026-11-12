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

impl PackArgs {
    pub fn is_remote(&self) -> bool {
        self.registry.is_some()
    }

    pub fn from_argv(argv: &[String]) -> Result<Self, PackError> {
        let mut args = Self::default();
        let mut positionals: Vec<String> = Vec::new();
        let mut i = 1;
        while i < argv.len() {
            match argv[i].as_str() {
                "--registry" | "--namespace" | "--tag" | "--auth" => {
                    i += 1;
                    let value = argv.get(i).ok_or(PackError::Usage)?.clone();
                    match argv[i - 1].as_str() {
                        "--registry" => args.registry = Some(value),
                        "--namespace" => args.namespace = Some(value),
                        "--tag" => args.tag = Some(value),
                        _ => args.auth = Some(value),
                    }
                }
                _ => positionals.push(argv[i].clone()),
            }
            i += 1;
        }
        if args.is_remote() {
            if args.tag.is_none() || positionals.len() != 1 {
                return Err(PackError::Usage);
            }
            args.output = PathBuf::from(&positionals[0]);
        } else {
            if positionals.len() != 2 {
                return Err(PackError::Usage);
            }
            args.manifest = Some(PathBuf::from(&positionals[0]));
            args.output = PathBuf::from(&positionals[1]);
        }
        Ok(args)
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
    if args.is_remote() {
        let _ = (&args.registry, &args.namespace, &args.tag, &args.auth);
        // TODO (L3) [adapters_ingestion]: streaming HTTPS client pulls the OCI manifest and layer tar chunks from the registry with bearer auth.; slop: FetchPending swallowed, registry manifest never fetched
        return Err(PackError::FetchPending);
    }
    let _ = &args.manifest;
    // TODO (L3) [adapters_ingestion]: read the local OCI manifest and layer tar chunks from disk.; slop: FetchPending swallowed, local manifest never read
    Err(PackError::FetchPending)
}

fn phase1_scan_headers(manifest: &OciManifest) -> Result<RadixTree, PackError> {
    // TODO (L3) [ad_01_radix_layer_unpack_whiteout_merge]: stream layer tar headers only into the radix tree; apply whiteouts in place; compute topology, inode count, chunk extents.; slop: image builds, whited-out path still in tree
    let _ = manifest;
    Err(PackError::Phase1Pending)
}

fn phase2_emit_erofs(
    args: &PackArgs,
    tree: &RadixTree,
    digest: &mut blake3::Hasher,
) -> Result<ErofsImage, PackError> {
    // TODO (L3) [ad_05_single_pass_erofs_blake3_digest]: stream layer payload bodies into CAS blobs, feeding each payload chunk to digest; emit finalized EROFS data and directory blocks from the phase 1 map.; slop: digest string exists, CAS blob name mismatch unnoticed
    let _ = (args, tree, digest);
    Err(PackError::EmitPending)
}

fn run(args: PackArgs) -> Result<ErofsImage, PackError> {
    let manifest = fetch_manifest(&args)?;
    let tree = phase1_scan_headers(&manifest)?;
    let mut digest = blake3::Hasher::new();
    let mut image = phase2_emit_erofs(&args, &tree, &mut digest)?;
    image.rootfs_digest = digest.finalize().to_hex().to_string();
    Ok(image)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let argv: Vec<String> = std::env::args().collect();
    let args = PackArgs::from_argv(&argv)?;
    let _image = run(args)?;
    Ok(())
}
