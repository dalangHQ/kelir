// see: cans/05-storage-plane.md#Storage-architecture
// see: cans/03-sdk-architecture.md#Rust-SDK
pub use kelir_interface::storage::FsReconcileRequest;
// see: cans/05-storage-plane.md#Storage-architecture
pub use kelir_engine::planes::storage::{IdmapConfig, WorkspaceEncapsulation};

// see: cans/05-storage-plane.md#Storage-architecture
// see: cans/03-sdk-architecture.md#Rust-SDK
#[derive(Debug, Clone)]
pub struct StoragePlaneConfig {
    pub idmap: IdmapConfig,
    pub shared_caches: Vec<String>,
    pub lock_broker: bool,
    pub encapsulation: WorkspaceEncapsulation,
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
