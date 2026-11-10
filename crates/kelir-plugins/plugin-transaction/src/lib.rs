// see: cans/plugins/transaction.md#Plugin-transaction
#![forbid(unsafe_code)]

// TODO (L3): tests/test_transaction.rs per cans/artifacts/config/project-structure.txt.
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use async_trait::async_trait;
use kelir_interface::plugin_model::LockBrokerCoordination;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{KelirPlugin, LifecycleCtx, LifecycleParticipant};
use plugin_db_proxy::{SqlProxyCtx, SqlProxyService};

pub mod service_transaction;
pub use service_transaction::{Savepoint, TransactionHandle, TransactionScope, TransactionService};

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.transaction.two_phase_commit
#[derive(Clone, Debug)]
pub struct TransactionConfig {
    pub timeout_ms: u64,
    pub auto_rollback_on_taint: bool,
    pub flush_fuse_before_commit: bool,
}

impl Default for TransactionConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 30_000,
            auto_rollback_on_taint: true,
            flush_fuse_before_commit: true,
        }
    }
}

// see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
#[derive(Clone, Debug)]
struct ActiveTransaction {
    handle: TransactionHandle,
    namespace: [u8; 16],
    scope: TransactionScope,
}

// see: cans/plugins/transaction.md#Temporal-savepoints
#[derive(Clone, Debug)]
struct RecordedSavepoint {
    savepoint: Savepoint,
    transaction: TransactionHandle,
}

// see: cans/plugins/transaction.md#Temporal-savepoints
fn savepoint_marker(savepoint: Savepoint) -> String {
    format!("kelir_sp_{}", savepoint.id)
}

// see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
#[derive(Default)]
pub(crate) struct PeerCoordination {
    // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
    sql_proxy: RwLock<Option<SqlProxyCtx>>,
    lock_broker: RwLock<Option<Arc<dyn LockBrokerCoordination>>>,
    active: RwLock<Option<ActiveTransaction>>,
    savepoints: RwLock<Vec<RecordedSavepoint>>,
    next_handle: AtomicU64,
    next_savepoint: AtomicU64,
    next_nonce: AtomicU64,
}

impl PeerCoordination {
    fn read_slot<T: Send + Sync + 'static + Clone>(
        slot: &RwLock<Option<T>>,
    ) -> Result<Option<T>, PluginError> {
        match slot.read() {
            Ok(guard) => Ok(guard.clone()),
            Err(_) => Err(PluginError::Halt("peer coordination slot poisoned".into())),
        }
    }

    fn store_slot<T: Send + Sync + 'static>(
        slot: &RwLock<Option<T>>,
        value: T,
    ) -> Result<(), PluginError> {
        match slot.write() {
            Ok(mut guard) => {
                *guard = Some(value);
                Ok(())
            }
            Err(_) => Err(PluginError::Halt("peer coordination slot poisoned".into())),
        }
    }

    fn sql_peer(&self) -> Result<Option<SqlProxyCtx>, PluginError> {
        Self::read_slot(&self.sql_proxy)
    }

    fn lock_broker(&self) -> Result<Option<Arc<dyn LockBrokerCoordination>>, PluginError> {
        Self::read_slot(&self.lock_broker)
    }

    fn active(&self) -> Result<Option<ActiveTransaction>, PluginError> {
        Self::read_slot(&self.active)
    }

    fn clear_active(&self) -> Result<(), PluginError> {
        match self.active.write() {
            Ok(mut guard) => {
                *guard = None;
            }
            Err(_) => return Err(PluginError::Halt("peer coordination slot poisoned".into())),
        }
        match self.savepoints.write() {
            Ok(mut guard) => {
                guard.clear();
                Ok(())
            }
            Err(_) => Err(PluginError::Halt("savepoint ledger poisoned".into())),
        }
    }

    pub(crate) fn require_active(
        &self,
        handle: &TransactionHandle,
    ) -> Result<ActiveTransaction, PluginError> {
        let active = self.active()?.ok_or_else(|| {
            PluginError::reject(
                "EINVAL",
                "no transaction scope is active on the coordinator",
            )
        })?;
        if active.handle.id != handle.id {
            return Err(PluginError::reject(
                "EINVAL",
                format!(
                    "handle {} does not match the active transaction {}",
                    handle.id, active.handle.id
                ),
            ));
        }
        Ok(active)
    }

    // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
    fn require_sql_peer(&self) -> Result<Option<SqlProxyCtx>, PluginError> {
        let peer = self.sql_peer()?;
        if peer.is_none()
            && self
                .active()?
                .is_some_and(|active| !active.scope.schemas.is_empty())
        {
            return Err(PluginError::reject(
                "EPOLICY",
                "transaction declares SQL schemas but no SqlProxyCtx peer is bound",
            ));
        }
        Ok(peer)
    }

    // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
    pub(crate) fn begin_scope(
        &self,
        scope: TransactionScope,
    ) -> Result<TransactionHandle, PluginError> {
        if let Some(active) = self.active()? {
            return Err(PluginError::reject(
                "EINVAL",
                format!(
                    "transaction {} is still active; commit or abort it before opening a new scope",
                    active.handle.id
                ),
            ));
        }
        let handle = TransactionHandle {
            id: self.next_handle.fetch_add(1, Ordering::AcqRel),
        };
        // TODO (L3): mint the transaction UUID namespace from the session's seeded entropy source.
        let namespace = u128::from(self.next_nonce.fetch_add(1, Ordering::AcqRel)).to_le_bytes();
        Self::store_slot(
            &self.active,
            ActiveTransaction {
                handle,
                namespace,
                scope,
            },
        )?;
        // TODO (L3): open the scope over the declared blast radius; out-of-scope mutations reject with EACCES or EAmbiguousMutation.
        Ok(handle)
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    async fn prepare_participants(&self) -> Result<(), PluginError> {
        if self.active()?.is_none() {
            return Ok(());
        }
        // TODO (L3): issue SQL pre-commit on the coordinated SqlProxy connections; participant failure here triggers global rollback with EPOLICY.
        let _ = self.require_sql_peer()?;
        // TODO (L3): pin advisory-lock acquisition to the declared physical host inodes via the coordinated lock broker.
        let _ = self.lock_broker()?;
        Ok(())
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    async fn issue_sql_commit(&self) -> Result<(), PluginError> {
        if self.active()?.is_none() {
            return Ok(());
        }
        match self.require_sql_peer()? {
            Some(peer) => peer.service().issue_commit().await,
            None => Ok(()),
        }
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    async fn issue_sql_rollback(&self) -> Result<(), PluginError> {
        if self.active()?.is_none() {
            return Ok(());
        }
        match self.require_sql_peer()? {
            Some(peer) => peer.service().issue_rollback().await,
            None => Ok(()),
        }
    }

    // see: cans/plugins/transaction.md#Temporal-savepoints
    pub(crate) fn next_savepoint_id(&self) -> u64 {
        self.next_savepoint.fetch_add(1, Ordering::AcqRel)
    }

    // see: cans/plugins/transaction.md#Temporal-savepoints
    pub(crate) fn record_savepoint(
        &self,
        savepoint: Savepoint,
        transaction: TransactionHandle,
    ) -> Result<(), PluginError> {
        // TODO (L3): snapshot the broker upper and tombstone state as the opaque savepoint blob per cans/plugins/transaction.md#Temporal-savepoints.
        match self.savepoints.write() {
            Ok(mut guard) => {
                guard.push(RecordedSavepoint {
                    savepoint,
                    transaction,
                });
                Ok(())
            }
            Err(_) => Err(PluginError::Halt("savepoint ledger poisoned".into())),
        }
    }

    // see: cans/plugins/transaction.md#Temporal-savepoints
    pub(crate) fn rollback_recorded_savepoint(
        &self,
        transaction: &TransactionHandle,
        savepoint: Savepoint,
    ) -> Result<(), PluginError> {
        // TODO (L3): restore the snapshotted broker upper and tombstone state and issue SQL ROLLBACK TO savepoint_marker(savepoint); plugin-db-proxy exposes no rollback-to-savepoint surface yet.
        let mut savepoints = match self.savepoints.write() {
            Ok(guard) => guard,
            Err(_) => return Err(PluginError::Halt("savepoint ledger poisoned".into())),
        };
        let index = savepoints
            .iter()
            .rposition(|recorded| {
                recorded.savepoint == savepoint && recorded.transaction == *transaction
            })
            .ok_or_else(|| {
                PluginError::reject(
                    "EINVAL",
                    format!(
                        "savepoint {} is not registered on transaction {}",
                        savepoint.id, transaction.id
                    ),
                )
            })?;
        savepoints.truncate(index + 1);
        Ok(())
    }

    // see: cans/plugins/transaction.md#Temporal-savepoints
    pub(crate) async fn inject_sql_savepoint(&self, marker: &str) -> Result<(), PluginError> {
        let active = self.active()?;
        if active.is_none() {
            return Ok(());
        }
        match self.require_sql_peer()? {
            Some(peer) => peer.service().inject_savepoint(marker).await,
            None => Ok(()),
        }
    }

    // see: cans/05-storage-plane.md#Host-inode-lock-broker
    async fn release_transaction_locks_scoped(&self) -> Result<(), PluginError> {
        // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
        let active = self.active()?;
        let broker = self.lock_broker()?;
        match (active, broker) {
            (Some(active), Some(broker)) => {
                broker.release_transaction_locks_for(active.namespace).await
            }
            _ => Ok(()),
        }
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    pub(crate) async fn abort_active(&self) -> Result<(), PluginError> {
        // TODO (L3): discard the broker upper file and tombstones through the driver-side session path.
        self.issue_sql_rollback().await?;
        self.release_transaction_locks_scoped().await?;
        self.clear_active()
    }
}

// see: cans/plugins/transaction.md#Service-exports-(transaction)
#[derive(Clone)]
pub struct TransactionCtx {
    coordination: Arc<PeerCoordination>,
}

impl TransactionCtx {
    // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
    pub fn bind_sql_proxy(&self, peer: SqlProxyCtx) -> Result<(), PluginError> {
        PeerCoordination::store_slot(&self.coordination.sql_proxy, peer)
    }

    // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
    pub fn bind_lock_broker(
        &self,
        broker: Arc<dyn LockBrokerCoordination>,
    ) -> Result<(), PluginError> {
        PeerCoordination::store_slot(&self.coordination.lock_broker, broker)
    }

    // see: cans/05-storage-plane.md#Host-inode-lock-broker
    pub fn active_namespace(&self) -> Result<Option<[u8; 16]>, PluginError> {
        Ok(self.coordination.active()?.map(|active| active.namespace))
    }

    pub(crate) fn coordination(&self) -> &PeerCoordination {
        &self.coordination
    }
}

// see: cans/plugins/transaction.md#Plugin-transaction
#[derive(Clone, Default)]
pub struct TransactionPlugin {
    config: TransactionConfig,
    coordination: Arc<PeerCoordination>,
}

impl TransactionPlugin {
    pub fn new(config: TransactionConfig) -> Self {
        Self {
            config,
            coordination: Arc::new(PeerCoordination::default()),
        }
    }

    // see: cans/plugins/transaction.md#Service-exports-(transaction)
    pub fn ctx(&self) -> TransactionCtx {
        TransactionCtx {
            coordination: Arc::clone(&self.coordination),
        }
    }

    // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
    pub fn bind_sql_proxy(&self, peer: SqlProxyCtx) -> Result<(), PluginError> {
        self.ctx().bind_sql_proxy(peer)
    }

    // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
    pub fn bind_lock_broker(
        &self,
        broker: Arc<dyn LockBrokerCoordination>,
    ) -> Result<(), PluginError> {
        self.ctx().bind_lock_broker(broker)
    }
}

// see: cans/plugins/transaction.md#Plugin-transaction
impl KelirPlugin for TransactionPlugin {
    // see: cans/plugins/transaction.md#Service-exports-(transaction)
    type Context = TransactionCtx;
    fn context(&self) -> Self::Context {
        self.ctx()
    }
    fn name(&self) -> &'static str {
        "transaction"
    }
    // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
    fn fork_state(
        &self,
        _child_session_id: &str,
    ) -> Box<dyn kelir_interface::plugin_model::AnyPlugin> {
        // see: cans/08-lifecycle.md#CoW-fork
        Box::new(Self::new(self.config.clone()))
    }
}

// see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
#[async_trait]
impl LifecycleParticipant for TransactionPlugin {
    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    async fn on_pre_commit(&self, ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        if self.config.flush_fuse_before_commit {
            // see: cans/08-lifecycle.md#Hypervisor-primitives
            ctx.freeze_vcpus().await?;
            ctx.drain_fuse(Duration::from_millis(self.config.timeout_ms))
                .await?;
        }
        self.coordination.prepare_participants().await?;
        Ok(())
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    async fn on_post_commit(
        &self,
        ctx: &mut LifecycleCtx<'_>,
        committed: bool,
    ) -> Result<(), PluginError> {
        if committed {
            ctx.commit_broker_upper().await?;
            self.coordination.issue_sql_commit().await?;
            self.coordination.clear_active()?;
        } else {
            ctx.discard_broker_upper().await?;
            self.coordination.abort_active().await?;
        }
        if self.config.flush_fuse_before_commit {
            ctx.unfreeze_vcpus().await?;
        }
        Ok(())
    }
}
