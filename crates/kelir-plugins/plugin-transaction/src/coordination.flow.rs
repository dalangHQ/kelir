// see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use kelir_interface::plugin_model::LockBrokerCoordination;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{
    HCons, HNil, KelirPlugin, LifecycleParticipant, MountScope, PipelineBinder, PluginDeps,
};
use plugin_db_proxy::{SqlProxyCtx, SqlProxyService};

use crate::TransactionConfig;
use crate::service_transaction::{Savepoint, TransactionScope, TransactionScopeHandle};

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
pub(crate) struct ActiveTransaction {
    handle: TransactionScopeHandle,
    namespace: [u8; 16],
    scope: TransactionScope,
}

// see: cans/plugins/transaction.md#Temporal-savepoints
#[derive(Clone, Debug)]
struct RecordedSavepoint {
    savepoint: Savepoint,
    transaction: TransactionScopeHandle,
}

// see: cans/plugins/transaction.md#Temporal-savepoints
pub(crate) fn savepoint_marker(savepoint: Savepoint) -> String {
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
    outbox: Mutex<TransactionOutbox>,
    next_handle: AtomicU64,
    next_savepoint: AtomicU64,
    next_nonce: AtomicU64,
}

// see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
#[derive(Debug, Default)]
pub(crate) struct TransactionOutbox {
    buffered: Vec<(u64, usize)>,
    used_bytes: usize,
    next_frame: u64,
}

// see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
const OUTBOX_CAPACITY_BYTES: usize = 67_108_864;

impl TransactionOutbox {
    // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
    fn buffer(&mut self, len: usize) -> Result<u64, PluginError> {
        let next = self
            .used_bytes
            .checked_add(len)
            .ok_or_else(|| PluginError::Halt("E2BIG: transaction outbox size overflow".into()))?;
        if next > OUTBOX_CAPACITY_BYTES {
            return Err(PluginError::Halt(
                "E2BIG: transaction outbox deferral buffer capacity exceeded".into(),
            ));
        }
        let id = self.next_frame;
        self.next_frame = self.next_frame.saturating_add(1);
        self.buffered.push((id, len));
        self.used_bytes = next;
        Ok(id)
    }

    // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
    fn release(&mut self) -> Vec<u64> {
        let ids: Vec<u64> = self.buffered.iter().map(|(id, _)| *id).collect();
        self.buffered.clear();
        self.used_bytes = 0;
        ids
    }

    // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
    fn discard(&mut self) -> Vec<u64> {
        self.release()
    }
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

    pub(crate) fn clear_active(&self) -> Result<(), PluginError> {
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
        handle: &TransactionScopeHandle,
    ) -> Result<ActiveTransaction, PluginError> {
        let active = self.active()?.ok_or_else(|| {
            PluginError::Halt(
                "EINVAL: no transaction scope is active on the coordinator".to_string(),
            )
        })?;
        if active.handle.id != handle.id {
            return Err(PluginError::Halt(format!(
                "EINVAL: handle {} does not match the active transaction {}",
                handle.id, active.handle.id
            )));
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
            return Err(PluginError::Halt(
                "EPOLICY: transaction declares SQL schemas but no SqlProxyCtx peer is bound"
                    .to_string(),
            ));
        }
        Ok(peer)
    }

    // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
    pub(crate) fn begin_scope(
        &self,
        scope: TransactionScope,
    ) -> Result<TransactionScopeHandle, PluginError> {
        if let Some(active) = self.active()? {
            return Err(PluginError::Halt(format!(
                "EINVAL: transaction {} is still active; commit or abort it before opening a new scope",
                active.handle.id
            )));
        }
        let handle = TransactionScopeHandle {
            id: self.next_handle.fetch_add(1, Ordering::AcqRel),
        };
        // TODO: [relational_transaction]: mint the transaction UUID namespace from the session's seeded entropy source; slop: UUID namespace hardcoded, ids collide across sessions (deferred to L3)
        let namespace = u128::from(self.next_nonce.fetch_add(1, Ordering::AcqRel)).to_le_bytes();
        Self::store_slot(
            &self.active,
            ActiveTransaction {
                handle,
                namespace,
                scope,
            },
        )?;
        // TODO: [rt_06_spatial_blast_radius_schema_scope]: open the scope over the declared blast radius; out-of-scope mutations reject with EACCES or EAmbiguousMutation; slop: scope never opened, out-of-scope mutations pass, private schemas queried (deferred to L3)
        Ok(handle)
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    pub(crate) async fn prepare_participants(&self) -> Result<(), PluginError> {
        if self.active()?.is_none() {
            return Ok(());
        }
        // TODO: [rt_03_2pc_pre_commit_vcpu_pause_drain]: issue SQL pre-commit on the coordinated SqlProxy connections; participant failure here triggers global rollback with EPOLICY; slop: pre-commit without vCPU pause, writes land mid-2PC, split-brain (deferred to L3)
        let _ = self.require_sql_peer()?;
        // TODO: [relational_transaction]: pin advisory-lock acquisition to the declared physical host inodes via the coordinated lock broker; slop: locks unpinned to host inodes, broker lock floats across files (deferred to L3)
        let _ = self.lock_broker()?;
        Ok(())
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    pub(crate) async fn issue_sql_commit(&self) -> Result<(), PluginError> {
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
        transaction: TransactionScopeHandle,
    ) -> Result<(), PluginError> {
        // TODO: [relational_transaction]: snapshot the broker upper and tombstone state as the opaque savepoint blob per cans/plugins/transaction.md#Temporal-savepoints; slop: savepoint blob empty, rollback restores nothing, state lost (deferred to L3)
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
        transaction: &TransactionScopeHandle,
        savepoint: Savepoint,
    ) -> Result<(), PluginError> {
        // TODO: [relational_transaction]: restore the snapshotted broker upper and tombstone state and issue SQL ROLLBACK TO savepoint_marker(savepoint); plugin-db-proxy exposes no rollback-to-savepoint surface yet; slop: restore unimplemented, ROLLBACK TO never issued, savepoint useless (deferred to L3)
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
                PluginError::Halt(format!(
                    "EINVAL: savepoint {} is not registered on transaction {}",
                    savepoint.id, transaction.id
                ))
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
    pub(crate) async fn release_transaction_locks_scoped(&self) -> Result<(), PluginError> {
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

    // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
    pub(crate) fn buffer_side_effect(&self, len: usize) -> Result<u64, PluginError> {
        let mut outbox = self
            .outbox
            .lock()
            .map_err(|_| PluginError::Halt("transaction outbox poisoned".into()))?;
        outbox.buffer(len)
    }

    // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
    pub(crate) fn release_outbox(&self) -> Result<Vec<u64>, PluginError> {
        let mut outbox = self
            .outbox
            .lock()
            .map_err(|_| PluginError::Halt("transaction outbox poisoned".into()))?;
        Ok(outbox.release())
    }

    // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
    pub(crate) fn discard_outbox(&self) -> Result<Vec<u64>, PluginError> {
        let mut outbox = self
            .outbox
            .lock()
            .map_err(|_| PluginError::Halt("transaction outbox poisoned".into()))?;
        Ok(outbox.discard())
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    pub(crate) async fn abort_active(&self) -> Result<(), PluginError> {
        // TODO: [relational_transaction]: discard the broker upper file and tombstones through the driver-side session path; slop: upper file and tombstones never discarded, storage leaks after commit (deferred to L3)
        self.issue_sql_rollback().await?;
        self.release_transaction_locks_scoped().await?;
        self.discard_outbox()?;
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

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
pub type TransactionHandle = TransactionCtx;

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
    fn default_scope(&self) -> MountScope {
        // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
        MountScope::default()
    }
    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        let lifecycle_stage: Arc<dyn LifecycleParticipant> = Arc::new(self.clone());
        pipeline.bind_lifecycle_scoped(lifecycle_stage, self.default_scope());
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

// see: cans/02-plugin-model.md#KelirPlugin-trait
impl PluginDeps for TransactionPlugin {
    type Deps = HCons<SqlProxyCtx, HNil>;
}
