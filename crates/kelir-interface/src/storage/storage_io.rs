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
    // TODO: [sp_08_reconciler_atomic_leaf_commit] fill in the L3 wave; StorageIo::write needs the driver/transport enclave; slop: in-place write, no temp+rename, inode unchanged
    fn write(&self, path: &Path, offset: u64, data: &[u8]) -> Result<u64, PluginError>;
    // see: cans/05-storage-plane.md#Host-CoW-broker
    // TODO: [sp_09_reconciler_tombstone_unlink_commit] fill in the L3 wave; StorageIo::mkdir needs the driver/transport enclave; slop: commit Ok, tombstone or file still on host disk
    fn mkdir(&self, path: &Path, mode: u32) -> Result<(), PluginError>;
    // see: cans/05-storage-plane.md#Host-CoW-broker
    // TODO: [sp_09_reconciler_tombstone_unlink_commit] fill in the L3 wave; StorageIo::unlink needs the driver/transport enclave; slop: commit Ok, tombstone or file still on host disk
    fn unlink(&self, path: &Path) -> Result<(), PluginError>;
    // see: cans/05-storage-plane.md#Host-CoW-broker
    // TODO: [sp_08_reconciler_atomic_leaf_commit] fill in the L3 wave; StorageIo::rename needs the driver/transport enclave; slop: in-place write, no temp+rename, inode unchanged
    fn rename(&self, from: &Path, to: &Path) -> Result<(), PluginError>;
}
