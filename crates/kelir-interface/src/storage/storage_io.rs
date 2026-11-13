// see: cans/05-storage-plane.md#Workspace-commit
// see: cans/05-storage-plane.md#Containment
// see: cans/05-storage-plane.md#Host-CoW-broker
use crate::plugin_model::PluginError;
use std::path::{Path, PathBuf};

pub struct RootHandle {
    pub root: PathBuf,
}

pub struct HostFileLock {
    pub path: PathBuf,
}

pub trait StorageIo: Send + Sync {
    fn open_root(&self, root: &Path) -> Result<RootHandle, PluginError>;
    fn atomic_rename(&self, temp: &Path, target: &Path) -> Result<(), PluginError>;
    fn lock_exclusive(&self, path: &Path) -> Result<HostFileLock, PluginError>;
    fn punch_hole(&self, path: &Path, offset: u64, len: u64) -> Result<(), PluginError>;
    fn advise_dontneed(&self, path: &Path) -> Result<(), PluginError>;
    fn remove_file(&self, path: &Path) -> Result<(), PluginError>;
    // see: cans/05-storage-plane.md#Host-CoW-broker
    fn write(&self, path: &Path, offset: u64, data: &[u8]) -> Result<u64, PluginError> {
        let _ = (path, offset, data);
        Err(PluginError::Halt(
            "TODO (L3) [sp_08_reconciler_atomic_leaf_commit]: StorageIo::write needs the driver/transport enclave; slop: in-place write, no temp+rename, inode unchanged".into(),
        ))
    }
    // see: cans/05-storage-plane.md#Host-CoW-broker
    fn mkdir(&self, path: &Path, mode: u32) -> Result<(), PluginError> {
        let _ = (path, mode);
        Err(PluginError::Halt(
            "TODO (L3) [sp_09_reconciler_tombstone_unlink_commit]: StorageIo::mkdir needs the driver/transport enclave; slop: commit Ok, tombstone or file still on host disk".into(),
        ))
    }
    // see: cans/05-storage-plane.md#Host-CoW-broker
    fn unlink(&self, path: &Path) -> Result<(), PluginError> {
        let _ = path;
        Err(PluginError::Halt(
            "TODO (L3) [sp_09_reconciler_tombstone_unlink_commit]: StorageIo::unlink needs the driver/transport enclave; slop: commit Ok, tombstone or file still on host disk".into(),
        ))
    }
    // see: cans/05-storage-plane.md#Host-CoW-broker
    fn rename(&self, from: &Path, to: &Path) -> Result<(), PluginError> {
        let _ = (from, to);
        Err(PluginError::Halt(
            "TODO (L3) [sp_08_reconciler_atomic_leaf_commit]: StorageIo::rename needs the driver/transport enclave; slop: in-place write, no temp+rename, inode unchanged".into(),
        ))
    }
}
