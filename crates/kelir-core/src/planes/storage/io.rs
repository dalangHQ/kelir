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
    fn open_root(&self, root: &Path) -> Result<RootHandle, kelir_interface::errors::PluginError>;
    fn atomic_rename(
        &self,
        temp: &Path,
        target: &Path,
    ) -> Result<(), kelir_interface::errors::PluginError>;
    fn lock_exclusive(
        &self,
        path: &Path,
    ) -> Result<HostFileLock, kelir_interface::errors::PluginError>;
    fn punch_hole(
        &self,
        path: &Path,
        offset: u64,
        len: u64,
    ) -> Result<(), kelir_interface::errors::PluginError>;
    fn advise_dontneed(&self, path: &Path) -> Result<(), kelir_interface::errors::PluginError>;
}

pub struct UnimplementedStorageIo;

impl StorageIo for UnimplementedStorageIo {
    fn open_root(&self, root: &Path) -> Result<RootHandle, kelir_interface::errors::PluginError> {
        // TODO (L3): openat2 RESOLVE_BENEATH in kelir-drivers/kelir-transport enclave.
        let _ = root;
        Err(kelir_interface::errors::PluginError::Halt(
            "TODO (L3): StorageIo::open_root needs the driver/transport enclave".into(),
        ))
    }

    fn atomic_rename(
        &self,
        temp: &Path,
        target: &Path,
    ) -> Result<(), kelir_interface::errors::PluginError> {
        // TODO (L3): renameat2 RENAME_NOREPLACE in kelir-drivers/kelir-transport enclave.
        let _ = (temp, target);
        Err(kelir_interface::errors::PluginError::Halt(
            "TODO (L3): StorageIo::atomic_rename needs the driver/transport enclave".into(),
        ))
    }

    fn lock_exclusive(
        &self,
        path: &Path,
    ) -> Result<HostFileLock, kelir_interface::errors::PluginError> {
        // TODO (L3): flock LOCK_EX in kelir-drivers/kelir-transport enclave.
        let _ = path;
        Err(kelir_interface::errors::PluginError::Halt(
            "TODO (L3): StorageIo::lock_exclusive needs the driver/transport enclave".into(),
        ))
    }

    fn punch_hole(
        &self,
        path: &Path,
        offset: u64,
        len: u64,
    ) -> Result<(), kelir_interface::errors::PluginError> {
        // TODO (L3): fallocate FALLOC_FL_PUNCH_HOLE in kelir-drivers/kelir-transport enclave.
        let _ = (path, offset, len);
        Err(kelir_interface::errors::PluginError::Halt(
            "TODO (L3): StorageIo::punch_hole needs the driver/transport enclave".into(),
        ))
    }

    fn advise_dontneed(&self, path: &Path) -> Result<(), kelir_interface::errors::PluginError> {
        // TODO (L3): posix_fadvise POSIX_FADV_DONTNEED in kelir-drivers/kelir-transport enclave.
        let _ = path;
        Err(kelir_interface::errors::PluginError::Halt(
            "TODO (L3): StorageIo::advise_dontneed needs the driver/transport enclave".into(),
        ))
    }
}
