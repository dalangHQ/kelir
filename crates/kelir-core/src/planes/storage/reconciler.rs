// see: cans/05-storage-plane.md#Workspace-commit
// see: cans/artifacts/code/reconciler.rs#reconcile_upperdir
use super::io::StorageIo;
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
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
        // TODO (L3): diff this walk against the broker's changed-entry state.
        let mut entries = Vec::new();
        let upper = self.broker_dir.join("upper");
        if upper.is_dir() {
            self.walk_broker_dir(&upper, &upper, &mut entries)?;
        }
        let mut tombstone_targets = self.classify_tombstones()?;
        entries.append(&mut tombstone_targets);
        Ok(entries)
    }

    pub fn classify_tombstones(
        &self,
    ) -> Result<Vec<PathBuf>, kelir_interface::plugin::PluginError> {
        let mut targets = Vec::new();
        let tombstones_dir = self.broker_dir.join("tombstones");
        if tombstones_dir.is_dir() {
            self.walk_broker_dir(&tombstones_dir, &tombstones_dir, &mut targets)?;
        }
        let tombstones = self.tombstones.read().map_err(|_| {
            kelir_interface::plugin::PluginError::Halt("tombstone lock poisoned".into())
        })?;
        for tombstone in tombstones.iter() {
            let target = self.host_target.join(tombstone);
            self.contain(&target)?;
            targets.push(target);
        }
        Ok(targets)
    }

    pub fn stage(
        &self,
        paths: &[PathBuf],
    ) -> Result<StagingGuard, kelir_interface::plugin::PluginError> {
        // TODO (L3): copy entries to .tmp.<nonce> staging files owned by StagingGuard behind StorageIo.
        for path in paths {
            self.contain(path)?;
        }
        let _ = self.io.open_root(&self.host_target)?;
        let _ = (&self.broker_dir, self.guard);
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO (L3): Reconciler::stage needs the StorageIo copy primitive".into(),
        ))
    }

    pub fn commit(&self) -> Result<CommitStats, kelir_interface::plugin::PluginError> {
        // TODO (L3): per-entry temp+rename via StorageIo::atomic_rename; Host Inode Broker pre-checks active host flock descriptors.
        let _ = self.detect()?;
        let _ = self.io.open_root(&self.host_target)?;
        let _ = (&self.broker_dir, self.guard);
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO (L3): Reconciler::commit needs the StorageIo rename primitive".into(),
        ))
    }

    // see: cans/05-storage-plane.md#Containment
    pub fn contain(&self, target: &Path) -> Result<(), kelir_interface::plugin::PluginError> {
        if target
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(kelir_interface::plugin::PluginError::reject(
                "EXDEV",
                format!("path traversal rejected: {target:?}"),
            ));
        }
        if !target.starts_with(&self.host_target) {
            return Err(kelir_interface::plugin::PluginError::reject(
                "EXDEV",
                format!("target escapes workspace root: {target:?}"),
            ));
        }
        for ancestor in target.ancestors() {
            if ancestor == self.host_target.as_path() {
                break;
            }
            if let Ok(meta) = fs::symlink_metadata(ancestor) {
                if meta.is_symlink() {
                    return Err(kelir_interface::plugin::PluginError::reject(
                        "EPERM",
                        format!("symlink component rejected: {ancestor:?}"),
                    ));
                }
            }
        }
        if target.exists() {
            let root = self.host_target.canonicalize()?;
            let resolved = target.canonicalize()?;
            if !resolved.starts_with(&root) {
                return Err(kelir_interface::plugin::PluginError::reject(
                    "EXDEV",
                    format!("resolved target escapes workspace root: {resolved:?}"),
                ));
            }
        }
        Ok(())
    }

    pub fn reconcile(
        &self,
        req: FsReconcileRequest,
    ) -> Result<CommitStats, kelir_interface::plugin::PluginError> {
        // TODO (L3): detect -> classify -> stage -> commit orchestration; idempotent replay.
        let _ = self.detect()?;
        let _ = self.classify_tombstones()?;
        let _ = (
            &self.broker_dir,
            &self.host_target,
            self.guard,
            req.cwd,
            req.atomic_staging,
            req.fsync,
        );
        Err(kelir_interface::plugin::PluginError::Halt(
            "TODO (L3): Reconciler::reconcile orchestration".into(),
        ))
    }

    // see: cans/05-storage-plane.md#Workspace-commit
    fn walk_broker_dir(
        &self,
        root: &Path,
        current: &Path,
        entries: &mut Vec<PathBuf>,
    ) -> Result<(), kelir_interface::plugin::PluginError> {
        for entry in fs::read_dir(current)? {
            let entry = entry?;
            let path = entry.path();
            let Ok(rel) = path.strip_prefix(root) else {
                continue;
            };
            let target = self.host_target.join(rel);
            if entry.file_type()?.is_dir() {
                self.walk_broker_dir(root, &path, entries)?;
                continue;
            }
            self.contain(&target)?;
            entries.push(target);
        }
        Ok(())
    }
}
