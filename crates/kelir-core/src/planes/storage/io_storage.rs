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
            "TODO (L3): StorageIo::write needs the driver/transport enclave".into(),
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
            "TODO (L3): StorageIo::mkdir needs the driver/transport enclave".into(),
        ))
    }
    // see: cans/05-storage-plane.md#Host-CoW-broker
    fn unlink(&self, path: &Path) -> Result<(), kelir_interface::plugin_model::PluginError> {
        let _ = path;
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): StorageIo::unlink needs the driver/transport enclave".into(),
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
            "TODO (L3): StorageIo::rename needs the driver/transport enclave".into(),
        ))
    }
}

pub struct UnimplementedStorageIo;

impl StorageIo for UnimplementedStorageIo {
    fn open_root(
        &self,
        root: &Path,
    ) -> Result<RootHandle, kelir_interface::plugin_model::PluginError> {
        // TODO (L3): openat2 RESOLVE_BENEATH in kelir-drivers/kelir-transport enclave.
        let _ = root;
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): StorageIo::open_root needs the driver/transport enclave".into(),
        ))
    }

    fn atomic_rename(
        &self,
        temp: &Path,
        target: &Path,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3): renameat2 RENAME_NOREPLACE in kelir-drivers/kelir-transport enclave.
        let _ = (temp, target);
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): StorageIo::atomic_rename needs the driver/transport enclave".into(),
        ))
    }

    fn lock_exclusive(
        &self,
        path: &Path,
    ) -> Result<HostFileLock, kelir_interface::plugin_model::PluginError> {
        // TODO (L3): flock LOCK_EX in kelir-drivers/kelir-transport enclave.
        let _ = path;
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): StorageIo::lock_exclusive needs the driver/transport enclave".into(),
        ))
    }

    fn punch_hole(
        &self,
        path: &Path,
        offset: u64,
        len: u64,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3): fallocate FALLOC_FL_PUNCH_HOLE in kelir-drivers/kelir-transport enclave.
        let _ = (path, offset, len);
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): StorageIo::punch_hole needs the driver/transport enclave".into(),
        ))
    }

    fn advise_dontneed(
        &self,
        path: &Path,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3): posix_fadvise POSIX_FADV_DONTNEED in kelir-drivers/kelir-transport enclave.
        let _ = path;
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): StorageIo::advise_dontneed needs the driver/transport enclave".into(),
        ))
    }

    fn remove_file(&self, path: &Path) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3): unlink via the open root handle in kelir-drivers/kelir-transport enclave.
        let _ = path;
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): StorageIo::remove_file needs the driver/transport enclave".into(),
        ))
    }
}
