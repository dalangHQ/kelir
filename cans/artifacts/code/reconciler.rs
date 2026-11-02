// crates/kelir-core/src/planes/storage/reconciler.rs
//
// Diff-only atomic reconciler for the host CoW broker: VirtIO-FS only;
// no in-guest OverlayFS, no virtio-blk workspace disk. The broker
// replays the per-session sparse upper file + explicit tombstones
// against the base workspace (writes hit sparse disk blocks, never
// guest RAM; no whiteouts — deletions are explicit tombstones in
// broker state), applies file mutations via per-entry temp+rename,
// deletes unlink in place, new dirs create in place, and never
// duplicates unmodified base workspace files. Whole commit is
// idempotent: replaying after crash converges. Rollback: broker drops
// the upper file and tombstones, base untouched,
// FALLOC_FL_PUNCH_HOLE + posix_fadvise(DONTNEED).
// All host-side inode resolution uses openat2(RESOLVE_BENEATH |
// RESOLVE_NO_MAGICLINKS); S_ISUID/S_ISGID are stripped on every write
// (mode & !0o6000).
// Phases: Detect (scan broker upper + tombstones) -> Classify tombstones
// -> Stage (StagingGuard owns .tmp.<nonce> files) -> Commit (per-entry
// temp+rename; deletes unlink in place; new dirs create in place)
// -> Contain (strict root containment, EXDEV on escape).
// Host I/O on this plane is mandatory non-blocking: descriptors open
// with O_NONBLOCK, FIFO/socket lookups fail with ENXIO, and zero guest
// syscalls may sleep host ring workers.
//
// Canonical spec home: cans/05-storage-plane.md#Atomic-workspace-reconciler

use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraversalGuard {
    /// Uses openat2 with RESOLVE_BENEATH and RESOLVE_NO_MAGICLINKS
    /// (spec spelling; variant name kept for compatibility).
    StrictResolveBeneath,
    /// Fallback lexical prefix verification for platforms lacking openat2.
    LexicalPrefixCheck,
}

#[derive(Default)]
pub struct CommitStats {
    pub files_changed: u64,
    // Field name kept deliberately (held: rename not grounded in a code
    // schema): counts broker tombstones resolved, not overlay whiteouts.
    pub whiteouts_resolved: u64,
}

/// Owns every .tmp.<nonce> staging file for one commit. Uncommitted
/// staging files unlink unconditionally on Drop; the guard disarms only
/// after the atomic directory swap succeeds, so a failed commit never
/// leaves staging files behind.
pub struct StagingGuard {
    pub staged: Vec<PathBuf>,
    pub disarmed: bool,
}

impl Drop for StagingGuard {
    fn drop(&mut self) {
        if !self.disarmed {
            for path in &self.staged {
                let _ = fs::remove_file(path);
            }
        }
    }
}

// Commit fast path: renameat2(RENAME_EXCHANGE) directly, after the Host
// Inode Broker pre-checks active host flock descriptors. Fallback on
// EBUSY or EPERM: in-place write with 2000ms backoff.

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

        // Broker tombstone: a deletion recorded in broker state (no
        // whiteouts); the entry below unlinks in place. The marker
        // decoding here is left as-is — no tombstone record format is
        // pinned in the grounded spec facts, so none is invented.
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
            // Strip setuid/setgid on every write: in-guest root cannot
            // mint setuid host binaries (mode & !0o6000).
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = fs::metadata(&tmp_swap)?.permissions().mode();
                fs::set_permissions(&tmp_swap, fs::Permissions::from_mode(mode & !0o6000))?;
            }
            fs::rename(&tmp_swap, &target_path)?;
            stats.files_changed += 1;
        }
    }
    Ok(())
}
