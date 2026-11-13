#![forbid(unsafe_code)]
// see: cans/05-storage-plane.md#Storage-architecture
#[path = "fuse.run.rs"]
mod fuse;
#[path = "lock_broker.run.rs"]
mod lock_broker;
#[path = "reconciler.run.rs"]
mod reconciler;
#[path = "storage.flow.rs"]
pub mod storage;

pub use crate::lock_broker::{
    LockBroker, LockBrokerWiring, LockGrant, LockKey, LockKind, LockRequest, LockState,
};
pub use crate::reconciler::{Reconciler, StagingGuard};
pub use crate::storage::{fuse_ops, fuse_ops_with_synthetic_files, reconciler};
