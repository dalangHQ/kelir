// see: cans/05-storage-plane.md#Workspace-commit
// see: cans/artifacts/code/reconciler.rs
use std::fs;
use std::path::{Path, PathBuf};

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

pub struct StagingGuard {
    pub staged: Vec<PathBuf>,
    pub disarmed: bool,
}

impl Drop for StagingGuard {
    fn drop(&mut self) {
        if !self.disarmed {
            for path in &self.staged {
                let _ = fs::remove_file(path);
            }
        }
    }
}

pub struct Reconciler {
    broker_dir: PathBuf,
    host_target: PathBuf,
    guard: TraversalGuard,
}

impl Reconciler {
    pub fn new(broker_dir: PathBuf, host_target: PathBuf, guard: TraversalGuard) -> Self {
        Self {
            broker_dir,
            host_target,
            guard,
        }
    }

    pub fn detect(&self) -> Result<Vec<PathBuf>, kelir_interface::errors::PluginError> {
        // TODO (L3): scan broker upper + tombstones for changed entries.
        let _ = (&self.broker_dir, &self.host_target, self.guard);
        Err(kelir_interface::errors::PluginError::Halt(
            "TODO: Reconciler::detect".into(),
        ))
    }

    pub fn classify_tombstones(
        &self,
    ) -> Result<Vec<PathBuf>, kelir_interface::errors::PluginError> {
        // TODO (L3): classify tombstone entries to unlink targets.
        let _ = (&self.broker_dir, &self.host_target, self.guard);
        Err(kelir_interface::errors::PluginError::Halt(
            "TODO: Reconciler::classify_tombstones".into(),
        ))
    }

    pub fn stage(
        &self,
        paths: &[PathBuf],
    ) -> Result<StagingGuard, kelir_interface::errors::PluginError> {
        // TODO (L3): copy entries to .tmp.<nonce> staging files owned by StagingGuard.
        let _ = (paths, &self.broker_dir, &self.host_target, self.guard);
        Err(kelir_interface::errors::PluginError::Halt(
            "TODO: Reconciler::stage".into(),
        ))
    }

    pub fn commit(&self) -> Result<CommitStats, kelir_interface::errors::PluginError> {
        // TODO (L3): per-entry temp+rename; openat2 resolution; non-blocking I/O; platform renameat2/flock FFI cannot live here (kelir-core forbids unsafe) — needs an enclave home.
        let _ = (&self.broker_dir, &self.host_target, self.guard);
        Err(kelir_interface::errors::PluginError::Halt(
            "TODO: Reconciler::commit".into(),
        ))
    }

    pub fn contain(&self, target: &Path) -> Result<(), kelir_interface::errors::PluginError> {
        // TODO (L3): strict root containment, EXDEV on escape.
        let _ = (target, &self.host_target, self.guard);
        Err(kelir_interface::errors::PluginError::Halt(
            "TODO: Reconciler::contain".into(),
        ))
    }

    pub fn reconcile(&self) -> Result<CommitStats, kelir_interface::errors::PluginError> {
        // TODO (L3): detect -> classify -> stage -> commit -> contain; idempotent replay.
        let _ = (&self.broker_dir, &self.host_target, self.guard);
        Err(kelir_interface::errors::PluginError::Halt(
            "TODO: Reconciler::reconcile".into(),
        ))
    }
}
