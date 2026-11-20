// see: cans/12-adapters.md#Ingestion-matrix
use std::path::PathBuf;

use crate::{ErofsImage, OciManifest, PackArgs, PackError, RadixTree};

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

fn fetch_manifest(args: &PackArgs) -> Result<OciManifest, PackError> {
    if args.is_remote() {
        let _ = (&args.registry, &args.namespace, &args.tag, &args.auth);
        // L3-GAP: [adapters_ingestion]: streaming HTTPS client pulls the OCI manifest and layer tar chunks from the registry with bearer auth.; slop: FetchPending swallowed, registry manifest never fetched
        return Err(PackError::FetchPending);
    }
    let _ = &args.manifest;
    // L3-GAP: [adapters_ingestion]: read the local OCI manifest and layer tar chunks from disk.; slop: FetchPending swallowed, local manifest never read
    Err(PackError::FetchPending)
}

fn phase1_scan_headers(manifest: &OciManifest) -> Result<RadixTree, PackError> {
    // L3-GAP: [ad_01_radix_layer_unpack_whiteout_merge]: stream layer tar headers only into the radix tree; apply whiteouts in place; compute topology, inode count, chunk extents.; slop: image builds, whited-out path still in tree
    let _ = manifest;
    Err(PackError::Phase1Pending)
}

fn phase2_emit_erofs(
    args: &PackArgs,
    tree: &RadixTree,
    digest: &mut blake3::Hasher,
) -> Result<ErofsImage, PackError> {
    // L3-GAP: [ad_05_single_pass_erofs_blake3_digest]: stream layer payload bodies into CAS blobs, feeding each payload chunk to digest; emit finalized EROFS data and directory blocks from the phase 1 map.; slop: digest string exists, CAS blob name mismatch unnoticed
    let _ = (args, tree, digest);
    Err(PackError::EmitPending)
}

pub fn run(args: PackArgs) -> Result<ErofsImage, PackError> {
    let manifest = fetch_manifest(&args)?;
    let tree = phase1_scan_headers(&manifest)?;
    let mut digest = blake3::Hasher::new();
    let mut image = phase2_emit_erofs(&args, &tree, &mut digest)?;
    image.rootfs_digest = digest.finalize().to_hex().to_string();
    Ok(image)
}

impl RadixTree {
    pub fn new() -> Self {
        Self::default()
    }
}
