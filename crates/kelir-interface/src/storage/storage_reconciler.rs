// see: cans/05-storage-plane.md#Workspace-commit
use crate::plugin_model::PluginError;

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

// see: cans/05-storage-plane.md#Workspace-commit
pub trait ReconcilerOps: Send + Sync {
    fn reconcile(&self, req: FsReconcileRequest) -> Result<CommitStats, PluginError>;
}
