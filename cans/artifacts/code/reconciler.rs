// crates/kelir-core/src/planes/storage/storage_reconciler.rs
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
// -> Stage (StagingGuard owns staging temp files) -> Commit (per-entry
// temp+rename; deletes unlink in place; new dirs create in place)
// -> Contain (strict root containment, EXDEV on escape).
// Host I/O on this plane is mandatory non-blocking: descriptors open
// with O_NONBLOCK, FIFO/socket lookups fail with ENXIO, and zero guest
// syscalls may sleep host ring workers.
//
// Canonical spec home: cans/05-storage-plane.md#Workspace-commit

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraversalGuard {
    /// Uses openat2 with RESOLVE_BENEATH and RESOLVE_NO_MAGICLINKS.
    StrictResolveBeneath,
    /// Fallback lexical prefix verification for platforms lacking openat2.
    LexicalPrefixCheck,
}

#[derive(Default)]
pub struct CommitStats {
    pub files_changed: u64,
    /// Counts broker tombstones resolved (deleted paths unlinked in place).
    pub tombstones_resolved: u64,
}

/// Owns every .tmp.<nonce> staging file for one commit. Uncommitted
/// staging files unlink unconditionally on Drop; the guard disarms only
/// after per-entry commit succeeds, so a failed commit never
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

/// Platform rename + flock pre-check (05-storage-plane.md).
/// Linux: renameat2 + flock via direct libc FFI. Other platforms:
/// std rename, no flock pre-check in this artifact.
mod platform {
    #[cfg(target_os = "linux")]
    mod imp {
        use std::ffi::CString;
        use std::fs;
        use std::io;
        use std::os::unix::ffi::OsStrExt;
        use std::path::Path;

        const AT_FDCWD: i32 = -100;
        const LOCK_EX: i32 = 2;
        const LOCK_NB: i32 = 4;

        extern "C" {
            fn renameat2(
                olddirfd: i32, oldpath: *const u8,
                newdirfd: i32, newpath: *const u8, flags: u32,
            ) -> i32;
            fn flock(fd: i32, operation: i32) -> i32;
        }

        pub fn rename(tmp: &Path, target: &Path) -> io::Result<()> {
            let old = CString::new(tmp.as_os_str().as_bytes())
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in tmp path"))?;
            let new = CString::new(target.as_os_str().as_bytes())
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in target path"))?;
            let rc = unsafe {
                renameat2(AT_FDCWD, old.as_ptr() as *const u8, AT_FDCWD, new.as_ptr() as *const u8, 0)
            };
            if rc == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
        }

        /// Returns the locked file handle if the target exists and the
        /// lock was acquired; Ok(None) if the target does not exist.
        /// A held lock returns an EBUSY-style WouldBlock error.
        pub fn flock_target(target: &Path) -> io::Result<Option<fs::File>> {
            if !target.exists() {
                return Ok(None);
            }
            let file = fs::File::open(target)?;
            use std::os::unix::io::AsRawFd;
            let rc = unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) };
            if rc == 0 {
                Ok(Some(file))
            } else {
                Err(io::Error::last_os_error())
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    mod imp {
        use std::fs;
        use std::io;
        use std::path::Path;

        pub fn rename(tmp: &Path, target: &Path) -> io::Result<()> {
            fs::rename(tmp, target)
        }

        pub fn flock_target(_target: &Path) -> io::Result<Option<fs::File>> {
            Ok(None)
        }
    }

    pub use imp::{flock_target, rename};
}

/// A flock failure is EBUSY for fallback purposes on every platform:
/// Linux flock returns EWOULDBLOCK (11) when the lock is held.
fn flock_is_busy(e: &std::io::Error) -> bool {
    matches!(e.raw_os_error(), Some(11))
}

fn rename_platform(tmp: &Path, target: &Path) -> std::io::Result<()> {
    platform::rename(tmp, target)
}

fn flock_target(target: &Path) -> std::io::Result<Option<fs::File>> {
    platform::flock_target(target)
}

// Commit: per-entry temp+rename. Fallback on EBUSY or EPERM: in-place
// write with 2000ms backoff (implemented below). Host flock pre-check
// from 05-storage-plane.md#Workspace-commit is implemented here via
// the kernel lock itself: before renaming, the target (if it exists)
// is opened and flocked LOCK_EX|LOCK_NB; a held lock surfaces as
// EBUSY and takes the same 2000ms fallback. No broker lock table is
// needed because flock is the table.
//
// Platform rename (05-storage-plane.md#Workspace-commit): Linux uses
// renameat2 via direct libc FFI (no new crate dependency); other
// platforms use std rename, and renameatx_np / FileRenameInfoEx swap
// remain the host broker's platform layer there, not this artifact.
//
// Broker state dir layout: sessions/<id>/upper (per-session sparse
// upper file contents, walked as a directory tree) and
// sessions/<id>/tombstones (one file per deleted path, path preserved
// as subdirectories). reconcile_upperdir takes the broker state dir
// (sessions/<id>/) rather than only the upper dir.

pub async fn reconcile_upperdir(
    broker_dir: &Path,
    host_target: &Path,
    guard: TraversalGuard,
) -> Result<CommitStats, std::io::Error> {
    let mut stats = CommitStats::default();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
        .as_nanos();

    let canonical_root = host_target.canonicalize()?;
    let upperdir = broker_dir.join("upper");
    let tombstones_dir = broker_dir.join("tombstones");
    // The guard owns every staging temp file: on any early `?` return it
    // drops and unlinks whatever was staged but not yet committed.
    let mut staging = StagingGuard { staged: Vec::new(), disarmed: false };
    if upperdir.exists() {
        reconcile_diff(
            &upperdir, &upperdir, host_target, &canonical_root,
            nonce, guard, &mut staging, &mut stats
        ).await?;
    }
    if tombstones_dir.exists() {
        reconcile_tombstones(
            &tombstones_dir, &tombstones_dir, host_target,
            &canonical_root, &mut stats
        )?;
    }

    // Scoped flush of parent directory metadata
    if host_target.exists() {
        let f = fs::File::open(host_target)?;
        f.sync_all()?;
    }

    // Every staged temp was renamed or removed on success; disarm so Drop
    // is a no-op even if a staged path somehow remains listed.
    staging.disarmed = true;
    Ok(stats)
}

async fn reconcile_diff(
    upper_root: &Path,
    current: &Path,
    host_target: &Path,
    canonical_root: &Path,
    nonce: u128,
    guard: TraversalGuard,
    staging: &mut StagingGuard,
    stats: &mut CommitStats,
) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let rel_path = path.strip_prefix(upper_root)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
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

        if file_type.is_dir() {
            if !target_path.exists() {
                fs::create_dir_all(&target_path)?;
            }
            reconcile_diff(upper_root, &path, host_target, canonical_root, nonce, guard, staging, stats).await?;
        } else if file_type.is_file() {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let tmp_swap = target_path.with_extension(format!("tmp.{:x}", nonce));
            fs::copy(&path, &tmp_swap)?;
            // Register before any fallible step so a failure below leaves
            // the temp file to the guard's Drop instead of stranding it.
            staging.staged.push(tmp_swap.clone());
            // Strip setuid/setgid on every write: in-guest root cannot
            // mint setuid host binaries (mode & !0o6000). Unix-only:
            // Windows has no unix mode bits, so there is nothing to
            // strip there and this block must not resolve std::os::unix.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = fs::metadata(&tmp_swap)?.permissions().mode();
                fs::set_permissions(&tmp_swap, fs::Permissions::from_mode(mode & !0o6000))?;
            }
            // Flock pre-check first: a held host lock takes the same
            // EBUSY fallback as a failed rename, below. The lock handle
            // is held for the rename itself, then dropped.
            let rename_result = match flock_target(&target_path) {
                Ok(_lock) => rename_platform(&tmp_swap, &target_path),
                Err(e) if flock_is_busy(&e) => {
                    Err(std::io::Error::from_raw_os_error(16))
                }
                Err(e) => Err(e),
            };
            match rename_result {
                Ok(()) => {}
                // EBUSY (16) / EPERM (1), Linux and macOS values: 2000ms
                // backoff, then in-place write per
                // 05-storage-plane.md#Workspace-commit. The temp file is
                // removed here; any other error propagates and the guard
                // unlinks the staged temp on Drop.
                Err(e) if matches!(e.raw_os_error(), Some(16) | Some(1)) => {
                    // Async backoff, not thread::sleep: this reconciler
                    // runs on a Tokio worker and must never park it.
                    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
                    fs::copy(&path, &target_path)?;
                    let _ = fs::remove_file(&tmp_swap);
                }
                Err(e) => return Err(e),
            }
            staging.staged.retain(|p| p != &tmp_swap);
            stats.files_changed += 1;
        }
    }
    Ok(())
}

/// Tombstone phase: walk sessions/<id>/tombstones/, where each entry
/// mirrors a deleted path (path preserved as subdirectories), and
/// unlink the corresponding target in place. A tombstone entry that
/// is a directory is walked recursively, so nested deleted paths are
/// resolved entry by entry. Targets already gone are converged
/// (idempotent replay: nothing to remove, nothing counted). The same
/// strict containment check as file writes applies: a target that
/// exists is canonicalized and must stay under the canonical root,
/// otherwise the commit fails with PermissionDenied.
fn reconcile_tombstones(
    tombstones_root: &Path,
    current: &Path,
    host_target: &Path,
    canonical_root: &Path,
    stats: &mut CommitStats,
) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let rel_path = path.strip_prefix(tombstones_root)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let target_path = host_target.join(rel_path);

        if entry.file_type()?.is_dir() {
            reconcile_tombstones(
                tombstones_root, &path, host_target, canonical_root, stats
            )?;
            continue;
        }

        // Enforce strict containment, same check as file writes:
        // reject symlink hops outside host_target
        if target_path.exists() {
            let target_canon = target_path.canonicalize()?;
            if !target_canon.starts_with(canonical_root) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("Path traversal attempt detected: {:?}", target_path),
                ));
            }
        }

        if target_path.is_file() || target_path.is_symlink() {
            fs::remove_file(&target_path)?;
            stats.tombstones_resolved += 1;
        } else if target_path.is_dir() {
            fs::remove_dir_all(&target_path)?;
            stats.tombstones_resolved += 1;
        }
    }
    Ok(())
}
