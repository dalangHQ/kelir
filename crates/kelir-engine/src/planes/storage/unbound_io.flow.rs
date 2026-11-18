// see: cans/05-storage-plane.md#Storage-architecture
use std::path::Path;

use kelir_interface::storage::{RootHandle, StorageIo};

pub(super) struct UnboundStorageIo;

impl StorageIo for UnboundStorageIo {
    fn open_root(
        &self,
        _root: &Path,
    ) -> Result<RootHandle, kelir_interface::plugin_model::PluginError> {
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "StorageIo unbound at StoragePlane::new; bind the kelir-storage-io ops in the composition root".into(),
        ))
    }

    fn atomic_rename(
        &self,
        _temp: &Path,
        _target: &Path,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "StorageIo unbound at StoragePlane::new; bind the kelir-storage-io ops in the composition root".into(),
        ))
    }

    fn lock_exclusive(
        &self,
        _path: &Path,
    ) -> Result<kelir_interface::storage::HostFileLock, kelir_interface::plugin_model::PluginError>
    {
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "StorageIo unbound at StoragePlane::new; bind the kelir-storage-io ops in the composition root".into(),
        ))
    }

    fn punch_hole(
        &self,
        _path: &Path,
        _offset: u64,
        _len: u64,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "StorageIo unbound at StoragePlane::new; bind the kelir-storage-io ops in the composition root".into(),
        ))
    }

    fn advise_dontneed(
        &self,
        _path: &Path,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "StorageIo unbound at StoragePlane::new; bind the kelir-storage-io ops in the composition root".into(),
        ))
    }

    fn remove_file(&self, _path: &Path) -> Result<(), kelir_interface::plugin_model::PluginError> {
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "StorageIo unbound at StoragePlane::new; bind the kelir-storage-io ops in the composition root".into(),
        ))
    }

    fn write(
        &self,
        _path: &Path,
        _offset: u64,
        _data: &[u8],
    ) -> Result<u64, kelir_interface::plugin_model::PluginError> {
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "StorageIo unbound at StoragePlane::new; bind the kelir-storage-io ops in the composition root".into(),
        ))
    }

    fn mkdir(
        &self,
        _path: &Path,
        _mode: u32,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "StorageIo unbound at StoragePlane::new; bind the kelir-storage-io ops in the composition root".into(),
        ))
    }

    fn unlink(&self, _path: &Path) -> Result<(), kelir_interface::plugin_model::PluginError> {
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "StorageIo unbound at StoragePlane::new; bind the kelir-storage-io ops in the composition root".into(),
        ))
    }

    fn rename(
        &self,
        _from: &Path,
        _to: &Path,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "StorageIo unbound at StoragePlane::new; bind the kelir-storage-io ops in the composition root".into(),
        ))
    }
}
