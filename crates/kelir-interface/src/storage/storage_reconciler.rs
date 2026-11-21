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
/// Failure policy for `reconcile()` when the host-side atomic rename or lock acquire
/// fails. Reconcile never falls back to a non-atomic in-place overwrite.
///
/// - `EBUSY`: retry the atomic rename with bounded backoff; the wait never exceeds the
///   bound given in `RetryBusyUntilMs` and never exceeds 2000ms, then the reconcile
///   aborts without writing.
/// - `EPERM`: abort fail-closed immediately. There is never an in-place overwrite
///   fallback: a failed atomic rename must not degrade into a non-atomic write, or
///   a guest crash mid-write would leave a torn workspace file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconcileFailurePolicy {
    /// Retry `EBUSY` conflicts with bounded backoff up to the given number of
    /// milliseconds (hard-capped at 2000ms), then abort without writing.
    RetryBusyUntilMs(u64),
    /// Abort immediately on any failure; no retries, no in-place overwrite, ever.
    AbortFailClosed,
}

// see: cans/05-storage-plane.md#Workspace-commit
pub trait ReconcilerOps: Send + Sync {
    /// Apply the reconciled workspace commit atomically: stage the writes out of tree,
    /// fsync per `FsReconcileRequest::fsync`, then rename into place.
    ///
    /// On `EBUSY` the reconcile retries the atomic rename with bounded backoff up to
    /// 2000ms (see `ReconcileFailurePolicy::RetryBusyUntilMs`); on `EPERM` it aborts
    /// fail-closed and never falls back to an in-place overwrite of the destination.
    fn reconcile(&self, req: FsReconcileRequest) -> Result<CommitStats, PluginError>;
}
