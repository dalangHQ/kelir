// see: cans/12-adapters.md#Ingestion-matrix
use crate::{ErofsImage, OciManifest, PackArgs, PackError, RadixTree};

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

pub fn run(args: PackArgs) -> Result<ErofsImage, PackError> {
    let manifest = fetch_manifest(&args)?;
    let tree = phase1_scan_headers(&manifest)?;
    let mut digest = blake3::Hasher::new();
    let mut image = phase2_emit_erofs(&args, &tree, &mut digest)?;
    image.rootfs_digest = digest.finalize().to_hex().to_string();
    Ok(image)
}
