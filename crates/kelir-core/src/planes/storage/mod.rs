// see: cans/05-storage-plane.md#Storage-architecture
// see: cans/05-storage-plane.md#Host-CoW-broker
// see: cans/05-storage-plane.md#Storage-quotas
pub mod fuse;
pub mod io;
pub mod lock_broker;
pub mod reconciler;
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
    io: Arc<dyn io::StorageIo>,
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
            io: Arc::new(io::UnimplementedStorageIo),
        }
    }

    pub fn with_io(mut self, storage_io: Arc<dyn io::StorageIo>) -> Self {
        self.io = storage_io;
        self
    }

    pub fn reconciler(
        &self,
        broker_dir: PathBuf,
        guard: reconciler::TraversalGuard,
    ) -> reconciler::Reconciler {
        reconciler::Reconciler::new(
            broker_dir,
            self.workspace_root.clone(),
            guard,
            self.io.clone(),
        )
    }

    pub fn lock_broker(
        &self,
        interposer: Arc<dyn kelir_interface::plugin::FsInterposer>,
    ) -> lock_broker::LockBroker {
        let _ = self;
        lock_broker::LockBroker::new(interposer)
    }

    pub fn fuse_worker(
        &self,
        receiver: std::sync::mpsc::Receiver<fuse::FuseRequest>,
        interposer: Arc<dyn kelir_interface::plugin::FsInterposer>,
        session_id: [u8; 16],
    ) -> fuse::FuseWorker {
        let _ = self;
        fuse::FuseWorker::new(receiver, interposer, session_id)
    }

    pub fn reconcile(&self) -> Result<(), kelir_interface::plugin::PluginError> {
        // TODO (L3): reconcile workspace_root against sparse_upper_path and tombstones via Reconciler.
        let _ = (
            &self.workspace_root,
            &self.sparse_upper_path,
            &self.tombstones,
        );
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO (L3): StoragePlane::reconcile needs the reconciler commit path".into(),
        ))
    }

    pub fn discard(&self) -> Result<(), kelir_interface::plugin::PluginError> {
        // TODO (L3): drop the sparse upper file and tombstones; base untouched.
        let _ = (&self.sparse_upper_path, &self.tombstones);
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO (L3): StoragePlane::discard needs the sparse upper drop".into(),
        ))
    }

    pub fn record_tombstone(
        &self,
        path: &Path,
    ) -> Result<(), kelir_interface::plugin::PluginError> {
        let mut tombstones = self.tombstones.write().map_err(|_| {
            kelir_interface::plugin::PluginError::Halt("tombstone lock poisoned".into())
        })?;
        tombstones.insert(path.to_path_buf());
        Ok(())
    }

    // see: cans/05-storage-plane.md#Storage-quotas
    pub fn check_quota(
        &self,
        bytes_to_write: u64,
        new_inodes: u64,
    ) -> Result<(), kelir_interface::plugin::PluginError> {
        let bytes = self
            .quota
            .allocated_bytes
            .load(Ordering::SeqCst)
            .checked_add(bytes_to_write)
            .ok_or_else(|| {
                kelir_interface::plugin::PluginError::Halt("quota byte arithmetic overflow".into())
            })?;
        if bytes > self.quota.max_bytes {
            return Err(kelir_interface::plugin::PluginError::Halt(
                "storage byte quota exceeded".into(),
            ));
        }
        let inodes = self
            .quota
            .allocated_inodes
            .load(Ordering::SeqCst)
            .checked_add(new_inodes)
            .ok_or_else(|| {
                kelir_interface::plugin::PluginError::Halt("quota inode arithmetic overflow".into())
            })?;
        if inodes > self.quota.max_inodes {
            return Err(kelir_interface::plugin::PluginError::Halt(
                "storage inode quota exceeded".into(),
            ));
        }
        Ok(())
    }

    pub fn punch_hole_sparse_upper(&self) -> Result<(), kelir_interface::plugin::PluginError> {
        // TODO (L3): FALLOC_FL_PUNCH_HOLE + posix_fadvise(DONTNEED) on sparse_upper_path.
        self.io.punch_hole(&self.sparse_upper_path, 0, u64::MAX)?;
        self.io.advise_dontneed(&self.sparse_upper_path)
    }
}
