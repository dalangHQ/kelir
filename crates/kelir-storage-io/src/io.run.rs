// see: cans/05-storage-plane.md#Workspace-commit
// see: cans/05-storage-plane.md#Containment
// see: cans/05-storage-plane.md#Host-CoW-broker
use kelir_interface::storage::{HostFileLock, RootHandle, StorageIo};
use std::path::Path;
use std::sync::Arc;

pub(crate) struct RealStorageIo;

pub(crate) fn build_storage_io() -> Arc<dyn StorageIo> {
    Arc::new(RealStorageIo)
}

impl StorageIo for RealStorageIo {
    fn open_root(
        &self,
        _root: &Path,
    ) -> Result<RootHandle, kelir_interface::plugin_model::PluginError> {
        // L3-GAP: [storage_plane]: openat2 RESOLVE_BENEATH in kelir-drivers/kelir-transport enclave; slop: open escapes root, symlink traversal unblocked
        // TODO (L3) [storage_plane]: open root through the driver/transport enclave and return its handle.; slop: root opened without the enclave, the handle carries no fd and later ops fail.
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [vr_10_virtio_fs_fuse_req_channel]: StorageIo::open_root needs the driver/transport enclave; slop: path-only handle returned, no fd observable, channel unproven".into(),
        ))
    }

    fn atomic_rename(
        &self,
        _temp: &Path,
        _target: &Path,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // L3-GAP: [sp_08_reconciler_atomic_leaf_commit]: renameat2 RENAME_NOREPLACE in kelir-drivers/kelir-transport enclave; slop: in-place write, no temp+rename, inode unchanged
        // TODO (L3) [storage_plane]: rename temp onto target atomically through the driver/transport enclave.; slop: rename runs in place, a crash leaves a torn target.
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [sp_08_reconciler_atomic_leaf_commit]: StorageIo::atomic_rename needs the driver/transport enclave; slop: in-place write, no temp+rename, inode unchanged".into(),
        ))
    }

    fn lock_exclusive(
        &self,
        _path: &Path,
    ) -> Result<HostFileLock, kelir_interface::plugin_model::PluginError> {
        // L3-GAP: [sp_15_host_inode_lock_broker_flock_bridge]: flock LOCK_EX in kelir-drivers/kelir-transport enclave; slop: in-memory lock table, no host flock, contender succeeds
        // TODO (L3) [storage_plane]: take the exclusive host flock on path through the driver/transport enclave.; slop: flock never taken, two writers commit the same path.
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [sp_15_host_inode_lock_broker_flock_bridge]: StorageIo::lock_exclusive needs the driver/transport enclave; slop: in-memory lock table, no host flock, contender succeeds".into(),
        ))
    }

    fn punch_hole(
        &self,
        _path: &Path,
        _offset: u64,
        _len: u64,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // L3-GAP: [sp_01_host_cow_upper_discard_on_rollback]: fallocate FALLOC_FL_PUNCH_HOLE in kelir-drivers/kelir-transport enclave; slop: rollback Ok, upper st_blocks nonzero, tombstones left
        // TODO (L3) [storage_plane]: punch a hole in path through the driver/transport enclave.; slop: hole never punched, rolled-back blocks stay allocated on disk.
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [sp_01_host_cow_upper_discard_on_rollback]: StorageIo::punch_hole needs the driver/transport enclave; slop: rollback Ok, upper st_blocks nonzero, tombstones left".into(),
        ))
    }

    fn advise_dontneed(
        &self,
        _path: &Path,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // L3-GAP: [sp_01_host_cow_upper_discard_on_rollback]: posix_fadvise POSIX_FADV_DONTNEED in kelir-drivers/kelir-transport enclave; slop: rollback Ok, upper st_blocks nonzero, tombstones left
        // TODO (L3) [storage_plane]: advise the kernel the path pages are unneeded through the driver/transport enclave.; slop: pages never advised away, reclaimed memory stays resident.
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [sp_01_host_cow_upper_discard_on_rollback]: StorageIo::advise_dontneed needs the driver/transport enclave; slop: rollback Ok, upper st_blocks nonzero, tombstones left".into(),
        ))
    }

    fn remove_file(&self, _path: &Path) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // L3-GAP: [sp_09_reconciler_tombstone_unlink_commit]: unlink via the open root handle in kelir-drivers/kelir-transport enclave; slop: commit Ok, tombstone or file still on host disk
        // TODO (L3) [storage_plane]: unlink path through the open root handle in the driver/transport enclave.; slop: unlink never runs, tombstoned files stay on host disk.
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [sp_09_reconciler_tombstone_unlink_commit]: StorageIo::remove_file needs the driver/transport enclave; slop: commit Ok, tombstone or file still on host disk".into(),
        ))
    }

    fn write(
        &self,
        _path: &Path,
        _offset: u64,
        _data: &[u8],
    ) -> Result<u64, kelir_interface::plugin_model::PluginError> {
        // TODO (L3) [storage_plane]: write data at offset in path through the driver/transport enclave.; slop: write lands in place, a crash mid-write leaves torn data.
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [sp_08_reconciler_atomic_leaf_commit]: StorageIo::write needs the driver/transport enclave; slop: in-place write, no temp+rename, inode unchanged".into(),
        ))
    }

    fn mkdir(
        &self,
        _path: &Path,
        _mode: u32,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3) [storage_plane]: create the directory at path with mode through the driver/transport enclave.; slop: directory never created, later writes fail into a missing path.
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [sp_09_reconciler_tombstone_unlink_commit]: StorageIo::mkdir needs the driver/transport enclave; slop: commit Ok, tombstone or file still on host disk".into(),
        ))
    }

    fn unlink(&self, _path: &Path) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3) [storage_plane]: unlink path through the driver/transport enclave.; slop: unlink never runs, deleted files stay on host disk.
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [sp_09_reconciler_tombstone_unlink_commit]: StorageIo::unlink needs the driver/transport enclave; slop: commit Ok, tombstone or file still on host disk".into(),
        ))
    }

    fn rename(
        &self,
        _from: &Path,
        _to: &Path,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3) [storage_plane]: rename from onto to through the driver/transport enclave.; slop: rename runs non-atomically, a crash leaves neither name valid.
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [sp_08_reconciler_atomic_leaf_commit]: StorageIo::rename needs the driver/transport enclave; slop: in-place write, no temp+rename, inode unchanged".into(),
        ))
    }
}
