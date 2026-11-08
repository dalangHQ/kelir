// see: cans/05-storage-plane.md#Workspace-commit
// see: cans/artifacts/code/reconciler.rs#reconcile_upperdir
use super::io::StorageIo;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraversalGuard {
    StrictResolveBeneath,
    LexicalPrefixCheck,
}

#[derive(Default)]
pub struct CommitStats {
    pub files_changed: u64,
    pub tombstones_resolved: u64,
}

#[derive(Debug, Clone)]
pub struct FsReconcileRequest {
    pub cwd: String,
    pub atomic_staging: bool,
    pub fsync: bool,
}

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
        if !self.disarmed {
            for path in &self.staged {
                match &self.io {
                    Some(io) => {
                        let _ = io.remove_file(path);
                    }
                    None => {
                        let _ = fs::remove_file(path);
                    }
                }
            }
        }
    }
}

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

    pub fn detect(&self) -> Result<Vec<PathBuf>, kelir_interface::plugin::PluginError> {
        // TODO (L3): scan broker upper + tombstones for changed entries.
        let _ = (
            &self.broker_dir,
            &self.host_target,
            self.guard,
            &self.tombstones,
        );
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO: Reconciler::detect".into(),
        ))
    }

    pub fn classify_tombstones(
        &self,
    ) -> Result<Vec<PathBuf>, kelir_interface::plugin::PluginError> {
        // TODO (L3): classify tombstone entries to unlink targets.
        let _ = (
            &self.broker_dir,
            &self.host_target,
            self.guard,
            &self.tombstones,
        );
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO: Reconciler::classify_tombstones".into(),
        ))
    }

    pub fn stage(
        &self,
        paths: &[PathBuf],
    ) -> Result<StagingGuard, kelir_interface::plugin::PluginError> {
        // TODO (L3): copy entries to .tmp.<nonce> staging files owned by StagingGuard.
        let _ = self.io.open_root(&self.host_target)?;
        let _ = (paths, &self.broker_dir, self.guard);
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO: Reconciler::stage".into(),
        ))
    }

    pub fn commit(&self) -> Result<CommitStats, kelir_interface::plugin::PluginError> {
        // TODO (L3): per-entry temp+rename via StorageIo::atomic_rename; Host Inode Broker pre-checks active host flock descriptors.
        let _ = self.io.open_root(&self.host_target)?;
        let _ = (&self.broker_dir, self.guard);
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO: Reconciler::commit".into(),
        ))
    }

    pub fn contain(&self, target: &Path) -> Result<(), kelir_interface::plugin::PluginError> {
        // TODO (L3): strict root containment, EXDEV on escape.
        let _ = self.io.open_root(&self.host_target)?;
        let _ = (target, self.guard);
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO: Reconciler::contain".into(),
        ))
    }

    pub fn reconcile(
        &self,
        req: FsReconcileRequest,
    ) -> Result<CommitStats, kelir_interface::plugin::PluginError> {
        // TODO (L3): detect -> classify -> stage -> commit -> contain; idempotent replay.
        let _ = self.detect()?;
        let _ = (
            &self.broker_dir,
            &self.host_target,
            self.guard,
            req.cwd,
            req.atomic_staging,
            req.fsync,
        );
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO: Reconciler::reconcile".into(),
        ))
    }
}
