// see: cans/05-storage-plane.md#Workspace-commit
// see: cans/artifacts/code/reconciler.rs#reconcile_upperdir
use kelir_interface::storage::{
    CommitStats, FsReconcileRequest, ReconcilerOps, StorageIo, TraversalGuard,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

pub struct StagingGuard {
    pub staged: Vec<PathBuf>,
    pub disarmed: bool,
    pub io: Option<Arc<dyn StorageIo>>,
}

impl StagingGuard {
    pub fn new(io: Arc<dyn StorageIo>) -> Self {
        Self {
            staged: Vec::new(),
            disarmed: false,
            io: Some(io),
        }
    }
}

impl Drop for StagingGuard {
    fn drop(&mut self) {
        // TODO (L3) [storage_plane]: remove every staged file through StorageIo, or the host filesystem when no io is bound.; slop: staged files never removed, aborted reconciles leave garbage in staging.
    }
}

pub(crate) fn build_reconciler(
    broker_dir: PathBuf,
    host_target: PathBuf,
    guard: TraversalGuard,
    io: Arc<dyn StorageIo>,
    tombstones: Arc<RwLock<HashSet<PathBuf>>>,
) -> Arc<dyn ReconcilerOps> {
    Arc::new(Reconciler::new(
        broker_dir,
        host_target,
        guard,
        io,
        tombstones,
    ))
}

// TODO (L3) [storage_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
pub struct Reconciler {
    broker_dir: PathBuf,
    host_target: PathBuf,
    guard: TraversalGuard,
    io: Arc<dyn StorageIo>,
    tombstones: Arc<RwLock<HashSet<PathBuf>>>,
}

impl Reconciler {
    pub fn new(
        broker_dir: PathBuf,
        host_target: PathBuf,
        guard: TraversalGuard,
        io: Arc<dyn StorageIo>,
        tombstones: Arc<RwLock<HashSet<PathBuf>>>,
    ) -> Self {
        Self {
            broker_dir,
            host_target,
            guard,
            io,
            tombstones,
        }
    }

    pub fn staging_guard(&self) -> StagingGuard {
        StagingGuard::new(Arc::clone(&self.io))
    }

    pub fn detect(&self) -> Result<Vec<PathBuf>, kelir_interface::plugin_model::PluginError> {
        // L3-GAP: [sp_08_reconciler_atomic_leaf_commit]: diff this walk against the broker's changed-entry state; slop: in-place write, no temp+rename, inode unchanged
        // TODO (L3) [storage_plane]: walk the broker upper and tombstone dirs and return the changed host targets.; slop: broker walk never runs, changed entries go undetected and the host drifts.
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [sp_08_reconciler_atomic_leaf_commit]: Reconciler::detect needs the broker walk and changed-entry diff".into(),
        ))
    }

    pub fn classify_tombstones(
        &self,
    ) -> Result<Vec<PathBuf>, kelir_interface::plugin_model::PluginError> {
        // TODO (L3) [storage_plane]: collect the tombstoned host targets from the tombstone dir and the tombstone set.; slop: tombstones never collected, deleted files resurrect on the next reconcile.
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): Reconciler::classify_tombstones needs the tombstone target collection"
                .into(),
        ))
    }

    pub fn stage(
        &self,
        _paths: &[PathBuf],
    ) -> Result<StagingGuard, kelir_interface::plugin_model::PluginError> {
        // L3-GAP: [sp_08_reconciler_atomic_leaf_commit]: copy entries to .tmp.<nonce> staging files owned by StagingGuard behind StorageIo; slop: in-place write, no temp+rename, inode unchanged
        // TODO (L3) [storage_plane]: copy the contained paths to staging files owned by a StagingGuard.; slop: paths copied without containment, staged writes escape the workspace root.
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [sp_08_reconciler_atomic_leaf_commit]: Reconciler::stage needs the StorageIo copy primitive; slop: in-place write, no temp+rename, inode unchanged".into(),
        ))
    }

    pub fn commit(&self) -> Result<CommitStats, kelir_interface::plugin_model::PluginError> {
        // L3-GAP: [sp_08_reconciler_atomic_leaf_commit]: per-entry temp+rename via StorageIo::atomic_rename; Host Inode Broker pre-checks active host flock descriptors; slop: in-place write, no temp+rename, inode unchanged
        // TODO (L3) [storage_plane]: commit the detected entries with per-entry temp+rename and return the commit stats.; slop: commit writes in place, a crash mid-commit leaves torn files.
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [sp_08_reconciler_atomic_leaf_commit]: Reconciler::commit needs the StorageIo rename primitive; slop: in-place write, no temp+rename, inode unchanged".into(),
        ))
    }

    // see: cans/05-storage-plane.md#Containment
    pub fn contain(
        &self,
        _target: &Path,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // L3-GAP: [storage_plane]: openat2 RESOLVE_BENEATH via the platform root handle removes the resolve race; slop: resolve race open, openat2 RESOLVE_BENEATH unused
        // TODO (L3) [storage_plane]: prove target stays beneath the workspace root with no parent-dir or symlink escape.; slop: containment never proven, a symlink target escapes the workspace root.
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): Reconciler::contain needs the containment check beneath the workspace root"
                .into(),
        ))
    }
}

impl ReconcilerOps for Reconciler {
    fn reconcile(
        &self,
        _req: FsReconcileRequest,
    ) -> Result<CommitStats, kelir_interface::plugin_model::PluginError> {
        // L3-GAP: [sp_08_reconciler_atomic_leaf_commit]: detect -> classify -> stage -> commit orchestration for non-empty broker state; idempotent replay; slop: in-place write, no temp+rename, inode unchanged
        // TODO (L3) [storage_plane]: run detect, classify, stage and commit in order for a non-empty broker state.; slop: orchestration never runs, reconcile reports success with nothing staged or committed.
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3) [sp_08_reconciler_atomic_leaf_commit]: Reconciler::reconcile orchestration for staged entries; slop: in-place write, no temp+rename, inode unchanged".into(),
        ))
    }
}

// TODO (L3) [storage_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
impl Reconciler {
    // see: cans/05-storage-plane.md#Workspace-commit
    fn walk_broker_dir(
        &self,
        _root: &Path,
        _current: &Path,
        _entries: &mut Vec<PathBuf>,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        // TODO (L3) [storage_plane]: walk current recursively and push the contained host targets into entries.; slop: recursive walk never runs, broker entries stay empty while the host changes.
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): Reconciler::walk_broker_dir needs the recursive broker walk".into(),
        ))
    }
}
