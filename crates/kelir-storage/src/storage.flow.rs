// see: cans/05-storage-plane.md#Storage-architecture
use crate::fuse::{build_fuse_ops, build_fuse_ops_with_files};
use crate::reconciler::build_reconciler;
use kelir_engine::dispatch_plugin::PipelineEngine;
use kelir_interface::storage::synthetic::SyntheticFile;
use kelir_interface::storage::{FuseOps, ReconcilerOps, StorageIo, TraversalGuard};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

pub fn fuse_ops(engine: Arc<PipelineEngine>) -> Arc<dyn FuseOps> {
    build_fuse_ops(engine)
}

pub fn fuse_ops_with_synthetic_files(
    engine: Arc<PipelineEngine>,
    files: Vec<SyntheticFile>,
) -> Arc<dyn FuseOps> {
    build_fuse_ops_with_files(engine, files)
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
