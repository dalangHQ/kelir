// crates/kelir-plugins/plugin-vfs/src/reconciler.rs
//
// Atomic Workspace Reconciler: stages upperdir changes into an isolated
// staging tree and executes an atomic directory swap into the host workspace.
//
// Canonical spec home: cans/05-storage-plane.md#Reconciler

use std::path::Path;

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
    let staging_dir = host_target.with_extension(format!("staging.{}", uuid::Uuid::new_v4()));

    if host_target.exists() {
        copy_dir_tree(host_target, &staging_dir)?;
    } else {
        std::fs::create_dir_all(&staging_dir)?;
    }

    for entry in walkdir::WalkDir::new(upperdir).min_depth(1) {
        let entry = entry?;
        let rel_path = entry.path().strip_prefix(upperdir).unwrap();
        let staged_target = staging_dir.join(rel_path);
        let metadata = entry.metadata()?;

        if metadata.is_file() {
            if let Some(parent) = staged_target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &staged_target)?;
            stats.files_changed += 1;
        }
    }

    // Atomic exchange into live workspace
    std::fs::rename(&staging_dir, host_target)?;

    // Scoped flush of parent directory metadata
    if let Some(parent) = host_target.parent() {
        let f = std::fs::File::open(parent)?;
        f.sync_all()?;
    }

    Ok(stats)
}

fn copy_dir_tree(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    for entry in walkdir::WalkDir::new(src) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src).unwrap();
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
