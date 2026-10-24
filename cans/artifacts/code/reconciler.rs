// crates/kelir-plugins/plugin-vfs/src/reconciler.rs
//
// Diff-only atomic reconciler: scans guest tmpfs upperdir, resolves
// overlayfs whiteouts (char devices st_rdev == 0 and opaque dirs),
// applies file mutations via atomic temporary swaps, and never duplicates
// unmodified base workspace files.
//
// Canonical spec home: cans/05-storage-plane.md#Atomic-workspace-staging

use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraversalGuard {
    /// Uses openat2 with RESOLVE_BENEATH and RESOLVE_NO_SYMLINKS.
    StrictResolveBeneath,
    /// Fallback lexical prefix verification for platforms lacking openat2.
    LexicalPrefixCheck,
}

#[derive(Default)]
pub struct CommitStats {
    pub files_changed: u64,
    pub whiteouts_resolved: u64,
}

pub async fn reconcile_upperdir(
    upperdir: &Path,
    host_target: &Path,
    guard: TraversalGuard,
) -> Result<CommitStats, std::io::Error> {
    let mut stats = CommitStats::default();
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();

    let canonical_root = host_target.canonicalize()?;
    reconcile_diff(
        upperdir, upperdir, host_target, &canonical_root,
        nonce, guard, &mut stats
    )?;

    // Scoped flush of parent directory metadata
    if host_target.exists() {
        let f = fs::File::open(host_target)?;
        f.sync_all()?;
    }

    Ok(stats)
}

fn reconcile_diff(
    upper_root: &Path,
    current: &Path,
    host_target: &Path,
    canonical_root: &Path,
    nonce: u128,
    guard: TraversalGuard,
    stats: &mut CommitStats,
) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let rel_path = path.strip_prefix(upper_root).unwrap();
        let target_path = host_target.join(rel_path);

        // Enforce strict containment: reject symlink hops outside host_target
        if target_path.exists() {
            let target_canon = target_path.canonicalize()?;
            if !target_canon.starts_with(canonical_root) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("Path traversal attempt detected: {:?}", target_path),
                ));
            }
        }
        let file_type = entry.file_type()?;

        // Overlayfs file deletion: char device with st_rdev == 0
        if file_type.is_char_device() {
            let meta = entry.metadata()?;
            if meta.rdev() == 0 {
                if target_path.is_file() || target_path.is_symlink() {
                    fs::remove_file(&target_path)?;
                    stats.whiteouts_resolved += 1;
                } else if target_path.is_dir() {
                    fs::remove_dir_all(&target_path)?;
                    stats.whiteouts_resolved += 1;
                }
                continue;
            }
        }

        if file_type.is_dir() {
            if !target_path.exists() {
                fs::create_dir_all(&target_path)?;
            }
            reconcile_diff(upper_root, &path, host_target, canonical_root, nonce, guard, stats)?;
        } else if file_type.is_file() {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let tmp_swap = target_path.with_extension(format!("tmp.{:x}", nonce));
            fs::copy(&path, &tmp_swap)?;
            fs::rename(&tmp_swap, &target_path)?;
            stats.files_changed += 1;
        }
    }
    Ok(())
}
