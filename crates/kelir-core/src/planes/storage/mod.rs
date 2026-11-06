// see: cans/05-storage-plane.md#Storage-architecture
// see: cans/05-storage-plane.md#Host-CoW-broker
// see: cans/05-storage-plane.md#Storage-quotas
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

pub struct StorageQuota {
    allocated_bytes: AtomicU64,
    allocated_inodes: AtomicU64,
    max_bytes: u64,
    max_inodes: u64,
}

pub struct StoragePlane {
    workspace_root: PathBuf,
    sparse_upper_path: PathBuf,
    tombstones: Arc<RwLock<HashSet<PathBuf>>>,
    quota: StorageQuota,
}

impl StoragePlane {
    pub fn new(
        workspace_root: PathBuf,
        sparse_upper: PathBuf,
        max_bytes: u64,
        max_inodes: u64,
    ) -> Self {
        Self {
            workspace_root,
            sparse_upper_path: sparse_upper,
            tombstones: Arc::new(RwLock::new(HashSet::new())),
            quota: StorageQuota {
                allocated_bytes: AtomicU64::new(0),
                allocated_inodes: AtomicU64::new(0),
                max_bytes,
                max_inodes,
            },
        }
    }

    pub fn reconcile(&self) -> Result<(), kelir_interface::errors::PluginError> {
        // TODO: reconcile workspace_root against sparse_upper_path and tombstones.
        let _ = (
            &self.workspace_root,
            &self.sparse_upper_path,
            &self.tombstones,
        );
        Ok(())
    }

    pub fn discard(&self) -> Result<(), kelir_interface::errors::PluginError> {
        // TODO: drop the sparse upper file and tombstones; base untouched.
        let _ = (&self.sparse_upper_path, &self.tombstones);
        Ok(())
    }

    pub fn record_tombstone(
        &self,
        path: &Path,
    ) -> Result<(), kelir_interface::errors::PluginError> {
        let mut tombstones = self.tombstones.write().map_err(|_| {
            kelir_interface::errors::PluginError::Halt("tombstone lock poisoned".into())
        })?;
        tombstones.insert(path.to_path_buf());
        Ok(())
    }

    // NOTE: check-only; no reservation is made here — reservation is L2.
    pub fn check_quota(
        &self,
        bytes_to_write: u64,
        new_inodes: u64,
    ) -> Result<(), kelir_interface::errors::PluginError> {
        let bytes = self
            .quota
            .allocated_bytes
            .load(Ordering::SeqCst)
            .checked_add(bytes_to_write)
            .ok_or_else(|| {
                kelir_interface::errors::PluginError::Halt("quota byte arithmetic overflow".into())
            })?;
        if bytes > self.quota.max_bytes {
            return Err(kelir_interface::errors::PluginError::Halt(
                "storage byte quota exceeded".into(),
            ));
        }
        let inodes = self
            .quota
            .allocated_inodes
            .load(Ordering::SeqCst)
            .checked_add(new_inodes)
            .ok_or_else(|| {
                kelir_interface::errors::PluginError::Halt("quota inode arithmetic overflow".into())
            })?;
        if inodes > self.quota.max_inodes {
            return Err(kelir_interface::errors::PluginError::Halt(
                "storage inode quota exceeded".into(),
            ));
        }
        Ok(())
    }

    pub fn punch_hole_sparse_upper(&self) -> Result<(), std::io::Error> {
        // TODO: FALLOC_FL_PUNCH_HOLE + posix_fadvise(DONTNEED) on sparse_upper_path.
        let _ = &self.sparse_upper_path;
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "TODO: StoragePlane::punch_hole_sparse_upper",
        ))
    }
}
