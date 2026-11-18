// see: cans/05-storage-plane.md#Containment
use super::IsolationError;
use std::path::{Path, PathBuf};

// see: cans/05-storage-plane.md#Containment
pub trait ContainedOpenOps: Send + Sync {
    // TODO (L3) [storage_plane]: open the resolved path with openat2(RESOLVE_BENEATH|RESOLVE_NO_MAGICLINKS) so a swapped symlink cannot escape the tenant root.; slop: resolve race open, symlink escape unchecked
    fn open_beneath(&self, resolved: &Path) -> Result<PathBuf, IsolationError>;
}
