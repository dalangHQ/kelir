// see: cans/05-storage-plane.md#Storage-architecture
// see: cans/03-sdk-architecture.md#Rust-SDK
use std::path::PathBuf;

pub use kelir_interface::storage::FsReconcileRequest;
// see: cans/05-storage-plane.md#Storage-architecture
pub use kelir_engine::planes::storage::{IdmapConfig, WorkspaceEncapsulation};

// see: cans/05-storage-plane.md#Storage-architecture
// TODO (L3) [plugin_engine]: real container image caching via external kelir-pack.; slop: cache hit fabricated, no kelir-pack digest or blob fetch
#[derive(Debug, Clone, Default)]
pub struct SharedCacheConfig {
    pub host_path: PathBuf,
    pub guest_mount: PathBuf,
    pub read_only: bool,
}

// see: cans/05-storage-plane.md#Storage-architecture
// see: cans/03-sdk-architecture.md#Rust-SDK
#[derive(Debug, Clone, Default)]
pub struct StoragePlaneConfig {
    pub idmap: IdmapConfig,
    pub shared_caches: Vec<SharedCacheConfig>,
    pub lock_broker: bool,
    pub encapsulation: WorkspaceEncapsulation,
    pub dax_window: Option<String>,
}

// see: cans/05-storage-plane.md#Host-inode-lock-broker
#[derive(Debug, Clone)]
pub struct LockBrokerConfig {
    pub scope: LockScope,
    pub non_blocking: bool,
}

// see: cans/05-storage-plane.md#Host-inode-lock-broker
#[derive(Debug, Clone)]
pub enum LockScope {
    Globs(Vec<String>),
}
