// see: cans/05-storage-plane.md#Storage-architecture
use crate::fuse::build_fuse_ops;
use crate::reconciler::build_reconciler;
use kelir_engine::dispatch_plugin::PipelineEngine;
use kelir_interface::storage::isolation_envelope::IsolationEnvelope;
use kelir_interface::storage::{FuseOps, ReconcilerOps, StorageIo, TraversalGuard};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

pub fn fuse_ops(engine: Arc<PipelineEngine>) -> Arc<dyn FuseOps> {
    build_fuse_ops(engine)
}

pub fn reconciler(
    broker_dir: PathBuf,
    host_target: PathBuf,
    guard: TraversalGuard,
    io: Arc<dyn StorageIo>,
    tombstones: Arc<RwLock<HashSet<PathBuf>>>,
) -> Arc<dyn ReconcilerOps> {
    build_reconciler(broker_dir, host_target, guard, io, tombstones)
}

// see: cans/01-architecture.md#Isolation-envelope
pub fn tenant_host_target(
    envelope: &IsolationEnvelope,
    session_id: &str,
    host_target: &Path,
) -> PathBuf {
    envelope
        .partition_dir(session_id)
        .join(host_target.strip_prefix("/").unwrap_or(host_target))
}

// see: cans/01-architecture.md#Isolation-envelope
pub fn traversal_guard(envelope: &IsolationEnvelope) -> TraversalGuard {
    if envelope.enforce_openat2_beneath {
        TraversalGuard::StrictResolveBeneath
    } else {
        TraversalGuard::LexicalPrefixCheck
    }
}
