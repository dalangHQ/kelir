// see: cans/05-storage-plane.md#Storage-architecture
// see: cans/05-storage-plane.md#Host-CoW-broker
// see: cans/05-storage-plane.md#Storage-quotas
#[path = "config.flow.rs"]
mod config;
#[path = "fuse.flow.rs"]
pub mod fuse;
#[path = "plane.flow.rs"]
mod plane;
#[path = "storage_quota.flow.rs"]
pub mod storage_quota;

pub use config::{IdmapConfig, LockBrokerWiring, StorageConfig, WorkspaceEncapsulation};
pub use plane::StoragePlane;
pub use storage_quota::StorageQuota;
