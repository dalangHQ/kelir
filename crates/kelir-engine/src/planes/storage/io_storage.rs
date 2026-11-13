// see: cans/05-storage-plane.md#Workspace-commit
// see: cans/05-storage-plane.md#Containment
// see: cans/05-storage-plane.md#Host-CoW-broker
use std::path::{Path, PathBuf};

pub struct RootHandle {
    pub root: PathBuf,
}

pub struct HostFileLock {
    pub path: PathBuf,
}

pub trait StorageIo: Send + Sync {
    fn open_root(
        &self,
        root: &Path,
    ) -> Result<RootHandle, kelir_interface::plugin_model::PluginError>;
    fn atomic_rename(
        &self,
        temp: &Path,
        target: &Path,
    ) -> Result<(), kelir_interface::plugin_model::PluginError>;
    fn lock_exclusive(
        &self,
        path: &Path,
    ) -> Result<HostFileLock, kelir_interface::plugin_model::PluginError>;
    fn punch_hole(
        &self,
        path: &Path,
        offset: u64,
        len: u64,
    ) -> Result<(), kelir_interface::plugin_model::PluginError>;
    fn advise_dontneed(
        &self,
        path: &Path,
    ) -> Result<(), kelir_interface::plugin_model::PluginError>;
    fn remove_file(&self, path: &Path) -> Result<(), kelir_interface::plugin_model::PluginError>;
    // see: cans/05-storage-plane.md#Host-CoW-broker
    fn write(
        &self,
        path: &Path,
        offset: u64,
        data: &[u8],
    ) -> Result<u64, kelir_interface::plugin_model::PluginError> {
        let _ = (path, offset, data);
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [sp_08_reconciler_atomic_leaf_commit]: StorageIo::write needs the driver/transport enclave; slop: in-place write, no temp+rename, inode unchanged".into(),
        ))
    }
    // see: cans/05-storage-plane.md#Host-CoW-broker
    fn mkdir(
        &self,
        path: &Path,
        mode: u32,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        let _ = (path, mode);
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [sp_09_reconciler_tombstone_unlink_commit]: StorageIo::mkdir needs the driver/transport enclave; slop: commit Ok, tombstone or file still on host disk".into(),
        ))
    }
    // see: cans/05-storage-plane.md#Host-CoW-broker
    fn unlink(&self, path: &Path) -> Result<(), kelir_interface::plugin_model::PluginError> {
        let _ = path;
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [sp_09_reconciler_tombstone_unlink_commit]: StorageIo::unlink needs the driver/transport enclave; slop: commit Ok, tombstone or file still on host disk".into(),
        ))
    }
    // see: cans/05-storage-plane.md#Host-CoW-broker
    fn rename(
        &self,
        from: &Path,
        to: &Path,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        let _ = (from, to);
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [sp_08_reconciler_atomic_leaf_commit]: StorageIo::rename needs the driver/transport enclave; slop: in-place write, no temp+rename, inode unchanged".into(),
        ))
    }
}

pub struct UnimplementedStorageIo;

impl StorageIo for UnimplementedStorageIo {
    fn open_root(
        &self,
        root: &Path,
    ) -> Result<RootHandle, kelir_interface::plugin_model::PluginError> {
        // TODO (L3) [storage_plane]: openat2 RESOLVE_BENEATH in kelir-drivers/kelir-transport enclave; slop: open escapes root, symlink traversal unblocked
        let _ = root;
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [vr_10_virtio_fs_fuse_req_channel]: StorageIo::open_root needs the driver/transport enclave; slop: path-only handle returned, no fd observable, channel unproven".into(),
        ))
    }

    fn atomic_rename(
        &self,
        temp: &Path,
        target: &Path,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3) [sp_08_reconciler_atomic_leaf_commit]: renameat2 RENAME_NOREPLACE in kelir-drivers/kelir-transport enclave; slop: in-place write, no temp+rename, inode unchanged
        let _ = (temp, target);
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [sp_08_reconciler_atomic_leaf_commit]: StorageIo::atomic_rename needs the driver/transport enclave; slop: in-place write, no temp+rename, inode unchanged".into(),
        ))
    }

    fn lock_exclusive(
        &self,
        path: &Path,
    ) -> Result<HostFileLock, kelir_interface::plugin_model::PluginError> {
        // TODO (L3) [sp_15_host_inode_lock_broker_flock_bridge]: flock LOCK_EX in kelir-drivers/kelir-transport enclave; slop: in-memory lock table, no host flock, contender succeeds
        let _ = path;
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [sp_15_host_inode_lock_broker_flock_bridge]: StorageIo::lock_exclusive needs the driver/transport enclave; slop: in-memory lock table, no host flock, contender succeeds".into(),
        ))
    }

    fn punch_hole(
        &self,
        path: &Path,
        offset: u64,
        len: u64,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3) [sp_01_host_cow_upper_discard_on_rollback]: fallocate FALLOC_FL_PUNCH_HOLE in kelir-drivers/kelir-transport enclave; slop: rollback Ok, upper st_blocks nonzero, tombstones left
        let _ = (path, offset, len);
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [sp_01_host_cow_upper_discard_on_rollback]: StorageIo::punch_hole needs the driver/transport enclave; slop: rollback Ok, upper st_blocks nonzero, tombstones left".into(),
        ))
    }

    fn advise_dontneed(
        &self,
        path: &Path,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3) [sp_01_host_cow_upper_discard_on_rollback]: posix_fadvise POSIX_FADV_DONTNEED in kelir-drivers/kelir-transport enclave; slop: rollback Ok, upper st_blocks nonzero, tombstones left
        let _ = path;
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [sp_01_host_cow_upper_discard_on_rollback]: StorageIo::advise_dontneed needs the driver/transport enclave; slop: rollback Ok, upper st_blocks nonzero, tombstones left".into(),
        ))
    }

    fn remove_file(&self, path: &Path) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3) [sp_09_reconciler_tombstone_unlink_commit]: unlink via the open root handle in kelir-drivers/kelir-transport enclave; slop: commit Ok, tombstone or file still on host disk
        let _ = path;
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [sp_09_reconciler_tombstone_unlink_commit]: StorageIo::remove_file needs the driver/transport enclave; slop: commit Ok, tombstone or file still on host disk".into(),
        ))
    }
}
