// see: cans/05-storage-plane.md#Workspace-commit
// see: cans/artifacts/code/reconciler.rs#reconcile_upperdir
use super::io_storage::StorageIo;
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

    pub fn detect(&self) -> Result<Vec<PathBuf>, kelir_interface::plugin_model::PluginError> {
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
    ) -> Result<Vec<PathBuf>, kelir_interface::plugin_model::PluginError> {
        let mut targets = Vec::new();
        let tombstones_dir = self.broker_dir.join("tombstones");
        if tombstones_dir.is_dir() {
            self.walk_broker_dir(&tombstones_dir, &tombstones_dir, &mut targets)?;
        }
        let tombstones = self.tombstones.read().map_err(|_| {
            kelir_interface::plugin_model::PluginError::Halt("tombstone lock poisoned".into())
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
    ) -> Result<StagingGuard, kelir_interface::plugin_model::PluginError> {
        // TODO (L3): copy entries to .tmp.<nonce> staging files owned by StagingGuard behind StorageIo.
        for path in paths {
            self.contain(path)?;
        }
        let _ = self.io.open_root(&self.host_target)?;
        let _ = (&self.broker_dir, self.guard);
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): Reconciler::stage needs the StorageIo copy primitive".into(),
        ))
    }

    pub fn commit(&self) -> Result<CommitStats, kelir_interface::plugin_model::PluginError> {
        // TODO (L3): per-entry temp+rename via StorageIo::atomic_rename; Host Inode Broker pre-checks active host flock descriptors.
        let _ = self.detect()?;
        let _ = self.io.open_root(&self.host_target)?;
        let _ = (&self.broker_dir, self.guard);
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): Reconciler::commit needs the StorageIo rename primitive".into(),
        ))
    }

    // see: cans/05-storage-plane.md#Containment
    pub fn contain(&self, target: &Path) -> Result<(), kelir_interface::plugin_model::PluginError> {
        if target
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(kelir_interface::plugin_model::PluginError::reject(
                kelir_interface::protocol_wire::Errno::Exdev,
                format!("path traversal rejected: {target:?}"),
            ));
        }
        if !target.starts_with(&self.host_target) {
            return Err(kelir_interface::plugin_model::PluginError::reject(
                kelir_interface::protocol_wire::Errno::Exdev,
                format!("target escapes workspace root: {target:?}"),
            ));
        }
        for ancestor in target.ancestors() {
            if ancestor == self.host_target.as_path() {
                break;
            }
            if fs::symlink_metadata(ancestor).is_ok_and(|meta| meta.is_symlink()) {
                return Err(kelir_interface::plugin_model::PluginError::reject(
                    kelir_interface::protocol_wire::Errno::Exdev,
                    format!("symlink component rejected: {ancestor:?}"),
                ));
            }
        }
        // TODO (L3): openat2 RESOLVE_BENEATH via the platform root handle removes the resolve race.
        let root = self.host_target.canonicalize()?;
        let mut prefix = target.to_path_buf();
        while prefix != self.host_target && !prefix.exists() {
            if !prefix.pop() {
                break;
            }
        }
        let resolved = prefix.canonicalize()?;
        if !resolved.starts_with(&root) {
            return Err(kelir_interface::plugin_model::PluginError::reject(
                kelir_interface::protocol_wire::Errno::Exdev,
                format!("resolved target escapes workspace root: {resolved:?}"),
            ));
        }
        Ok(())
    }

    pub fn reconcile(
        &self,
        req: FsReconcileRequest,
    ) -> Result<CommitStats, kelir_interface::plugin_model::PluginError> {
        // TODO (L3): detect -> classify -> stage -> commit orchestration for non-empty broker state; idempotent replay.
        let entries = self.detect()?;
        let _ = (
            &self.broker_dir,
            &self.host_target,
            self.guard,
            req.cwd,
            req.atomic_staging,
            req.fsync,
        );
        if entries.is_empty() {
            return Ok(CommitStats::default());
        }
        Err(kelir_interface::plugin_model::PluginError::Halt(
            "TODO (L3): Reconciler::reconcile orchestration for staged entries".into(),
        ))
    }

    // see: cans/05-storage-plane.md#Workspace-commit
    fn walk_broker_dir(
        &self,
        root: &Path,
        current: &Path,
        entries: &mut Vec<PathBuf>,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
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
