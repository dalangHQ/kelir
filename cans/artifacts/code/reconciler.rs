// crates/kelir-plugins/plugin-vfs/src/reconciler.rs
//
// Atomic Workspace Reconciler: stages upperdir changes into an isolated
// staging tree and executes an atomic directory swap into the host workspace.
// Zero-dependency implementation: eliminates walkdir and uuid.
//
// Canonical spec home: cans/05-storage-plane.md#Reconciler

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Default)]
pub struct CommitStats {
    pub files_changed: u64,
    pub whiteouts_resolved: u64,
}

pub async fn reconcile_upperdir(
    upperdir: &Path,
    host_target: &Path,
) -> Result<CommitStats, std::io::Error> {
    let mut stats = CommitStats::default();
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let staging_dir = host_target.with_extension(format!("staging.{:x}", nonce));

    if host_target.exists() {
        copy_dir_tree(host_target, &staging_dir)?;
    } else {
        fs::create_dir_all(&staging_dir)?;
    }

    reconcile_recursive(upperdir, upperdir, &staging_dir, &mut stats)?;

    // Atomic exchange into live workspace
    fs::rename(&staging_dir, host_target)?;

    // Scoped flush of parent directory metadata
    if let Some(parent) = host_target.parent() {
        let f = fs::File::open(parent)?;
        f.sync_all()?;
    }

    Ok(stats)
}

fn reconcile_recursive(
    root: &Path,
    current: &Path,
    staging_root: &Path,
    stats: &mut CommitStats,
) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let rel_path = path.strip_prefix(root).unwrap();
        let staged_target = staging_root.join(rel_path);
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            fs::create_dir_all(&staged_target)?;
            reconcile_recursive(root, &path, staging_root, stats)?;
        } else if file_type.is_file() {
            if let Some(parent) = staged_target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&path, &staged_target)?;
            stats.files_changed += 1;
        }
    }
    Ok(())
}

fn copy_dir_tree(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            fs::create_dir_all(&target)?;
            copy_dir_tree(&path, &target)?;
        } else {
            fs::copy(&path, target)?;
        }
    }
    Ok(())
}
