// crates/kelir-plugins/plugin-transaction/src/service_transaction.rs
//
// The TransactionService surface: what the transaction plugin exposes
// via PipelineBinder for multi-resource 2PC management. The service
// coordinates commit/abort across the host CoW broker, db-proxy, and
// lock broker, and with hypervisor lifecycle (vCPUs pause during
// commit evaluation). The lifecycle commit hooks are the service's
// mechanism: on_pre_commit pauses guest vCPUs and drains FUSE
// in-flight ops; on_post_commit(true) invokes the broker commit and
// issues SQL COMMIT; on_post_commit(false) discards the broker upper
// file and tombstones, issues SQL ROLLBACK, and releases advisory
// locks. Deadline and taint policy live in the profile, not here:
// timeout_ms and auto_rollback_on_taint are
// kelir.yaml#plugins.transaction.two_phase_commit.
//
// Owned by: cans/plugins/transaction.md#Service-exports-(transaction)

use async_trait::async_trait;
use std::path::PathBuf;
use kelir_interface::plugin::PluginError;

/// Declared blast radius of one transaction. A mutation outside the
/// declared scope fails closed: filesystem writes outside `paths`
/// and queries outside `schemas` reject with EACCES, and advisory
/// lock acquisition is restricted to the declared physical host
/// inodes in `inodes`.
#[derive(Debug, Clone, Default)]
pub struct TransactionScope {
    /// Filesystem path subtrees the transaction may mutate.
    pub paths: Vec<PathBuf>,
    /// Database schemas the transaction may query, enforced by the
    /// pgwire frame filter.
    pub schemas: Vec<String>,
    /// Physical host inodes the transaction may lock.
    pub inodes: Vec<u64>,
}

/// Opaque handle for one open transaction. Commit and abort act on
/// the handle; handles never cross a fork boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransactionHandle {
    pub id: u64,
}

/// Opaque savepoint blob: a snapshot of broker upper and tombstone
/// state. Rolling back to a savepoint restores intermediate state
/// without aborting the parent transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Savepoint {
    pub id: u64,
}

/// Multi-resource two-phase commit coordination.
#[async_trait]
pub trait TransactionService: Send + Sync {
    /// Opens a transaction over the declared blast radius. A
    /// participant failure during pre-commit triggers global
    /// rollback with EPOLICY (PluginError::reject).
    async fn begin_scope(
        &mut self,
        scope: TransactionScope,
    ) -> Result<TransactionHandle, PluginError>;

    /// Commits the transaction: the broker replays the upper file
    /// plus tombstones against the base workspace and SQL COMMIT is
    /// issued across the active proxy connections.
    async fn commit(&mut self, handle: TransactionHandle) -> Result<(), PluginError>;

    /// Aborts the transaction: the broker upper file and tombstones
    /// are discarded, SQL ROLLBACK is issued, and advisory locks
    /// are released. An uncommitted transaction that exceeds the
    /// profile timeout_ms aborts the same way with ETIMEDOUT.
    async fn abort(&mut self, handle: TransactionHandle) -> Result<(), PluginError>;

    /// Injects a transparent SQL SAVEPOINT marker into the active
    /// pgwire connections and snapshots broker upper and tombstone
    /// state as an opaque savepoint blob.
    async fn savepoint(
        &mut self,
        handle: &TransactionHandle,
    ) -> Result<Savepoint, PluginError>;

    /// Rolls back to the savepoint, restoring intermediate state
    /// without aborting the parent transaction.
    async fn rollback_to_savepoint(
        &mut self,
        handle: &TransactionHandle,
        savepoint: Savepoint,
    ) -> Result<(), PluginError>;
}
