// see: cans/05-storage-plane.md#Workspace-commit
use crate::io::build_storage_io;
use kelir_interface::storage::StorageIo;
use std::sync::Arc;

pub fn storage_io() -> Arc<dyn StorageIo> {
    build_storage_io()
}
