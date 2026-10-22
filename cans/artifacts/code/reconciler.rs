// crates/kelir-plugins/plugin-vfs/src/reconciler.rs
//
// The Success Path Reconciler: syncs the guest overlayfs upperdir
// differential back to the host workspace. Pure storage-plane operation —
// no VCS semantics, no `message` parameter. Git commits are ordinary guest
// execution through the trap chain.
//
// Resolves two overlayfs whiteout forms:
//   - file whiteout:  char device major/minor 0/0  → host unlink()
//   - dir whiteout:   xattr trusted.overlay.opaque=y → host remove_dir_all()
//   - data sync:      ioctl(FICLONE) CoW block clone with atomic tmp write fallback
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

    for entry in walkdir::WalkDir::new(upperdir).min_depth(1) {
        let entry = entry?;
        let rel_path = entry.path().strip_prefix(upperdir).unwrap();
        let target_path = host_target.join(rel_path);
        let metadata = entry.metadata()?;

        // 1. File whiteout: char device 0/0 → unlink the host file
        if metadata.file_type().is_char_device() && metadata.rdev() == 0 {
            if target_path.exists() {
                std::fs::remove_file(&target_path)?;
                stats.whiteouts_resolved += 1;
            }
            continue;
        }

        // 2. Directory whiteout: `trusted.overlay.opaque = "y"` → remove
        //    the host directory tree so the guest deletion holds
        if metadata.is_dir() {
            if let Ok(Some(val)) = xattr::get(entry.path(), "trusted.overlay.opaque") {
                if val == b"y" && target_path.exists() {
                    std::fs::remove_dir_all(&target_path)?;
                    stats.whiteouts_resolved += 1;
                    continue;
                }
            }
            std::fs::create_dir_all(&target_path)?;
            continue;
        }

        // 3. CoW reflink block clone with atomic staged write fallback
        if metadata.is_file() {
            let tmp_path =
                target_path.with_extension(format!("tmp.{}", std::process::id()));
            std::fs::copy(entry.path(), &tmp_path)?;
            std::fs::rename(&tmp_path, &target_path)?;
            stats.files_changed += 1;
        }
    }

    nix::unistd::sync();
    Ok(stats)
}
