// see: cans/plugins/transaction.md
#![forbid(unsafe_code)]

// TODO (L3): tests/test_transaction.rs per cans/artifacts/config/project-structure.txt.
use std::sync::{Arc, RwLock};
use std::time::Duration;

use async_trait::async_trait;
use kelir_interface::plugin::LockBrokerCoordination;
use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{
    KelirPlugin, LifecycleCtx, LifecycleParticipant, MountScope, PipelineBinder,
};
use kelir_interface::service_registry::ServiceRegistry;
use plugin_db_proxy::SqlProxyService;

pub mod service;
pub use service::{Savepoint, TransactionHandle, TransactionScope, TransactionService};

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

#[derive(Default)]
struct PeerCoordination {
    // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
    lock_broker: RwLock<Option<Arc<dyn LockBrokerCoordination>>>,
    lock_namespace: RwLock<Option<[u8; 16]>>,
    sql_proxy: RwLock<Option<Arc<dyn SqlProxyService>>>,
}

impl PeerCoordination {
    fn read_slot<T: ?Sized + Send + Sync + 'static>(
        slot: &RwLock<Option<Arc<T>>>,
    ) -> Result<Option<Arc<T>>, PluginError> {
        match slot.read() {
            Ok(guard) => Ok(guard.clone()),
            Err(_) => Err(PluginError::Halt("peer coordination slot poisoned".into())),
        }
    }

    fn store_slot<T: ?Sized + Send + Sync + 'static>(
        slot: &RwLock<Option<Arc<T>>>,
        service: Arc<T>,
    ) -> Result<(), PluginError> {
        match slot.write() {
            Ok(mut guard) => {
                *guard = Some(service);
                Ok(())
            }
            Err(_) => Err(PluginError::Halt("peer coordination slot poisoned".into())),
        }
    }

    fn lock_broker(&self) -> Result<Option<Arc<dyn LockBrokerCoordination>>, PluginError> {
        Self::read_slot(&self.lock_broker)
    }

    fn lock_namespace(&self) -> Result<Option<[u8; 16]>, PluginError> {
        match self.lock_namespace.read() {
            Ok(guard) => Ok(*guard),
            Err(_) => Err(PluginError::Halt("peer coordination slot poisoned".into())),
        }
    }

    fn set_lock_namespace(&self, namespace: [u8; 16]) -> Result<(), PluginError> {
        match self.lock_namespace.write() {
            Ok(mut guard) => {
                *guard = Some(namespace);
                Ok(())
            }
            Err(_) => Err(PluginError::Halt("peer coordination slot poisoned".into())),
        }
    }

    fn sql_proxy(&self) -> Result<Option<Arc<dyn SqlProxyService>>, PluginError> {
        Self::read_slot(&self.sql_proxy)
    }

    fn set_lock_broker(&self, service: Arc<dyn LockBrokerCoordination>) -> Result<(), PluginError> {
        Self::store_slot(&self.lock_broker, service)
    }

    fn set_sql_proxy(&self, service: Arc<dyn SqlProxyService>) -> Result<(), PluginError> {
        Self::store_slot(&self.sql_proxy, service)
    }
}

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

    fn peer_lock_broker(&self) -> Result<Option<Arc<dyn LockBrokerCoordination>>, PluginError> {
        self.coordination.lock_broker()
    }

    fn peer_sql_proxy(&self) -> Result<Option<Arc<dyn SqlProxyService>>, PluginError> {
        self.coordination.sql_proxy()
    }

    // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
    pub fn bind_lock_broker(
        &self,
        service: Arc<dyn LockBrokerCoordination>,
    ) -> Result<(), PluginError> {
        self.coordination.set_lock_broker(service)
    }

    // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
    pub fn bind_sql_proxy(&self, service: Arc<dyn SqlProxyService>) -> Result<(), PluginError> {
        self.coordination.set_sql_proxy(service)
    }

    // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
    pub fn resolve_peers(&self, registry: &ServiceRegistry) -> Result<(), PluginError> {
        if let Some(sql) = registry.lookup::<dyn SqlProxyService>() {
            self.coordination.set_sql_proxy(sql)?;
        }
        if let Some(locks) = registry.lookup::<dyn LockBrokerCoordination>() {
            self.coordination.set_lock_broker(locks)?;
        }
        Ok(())
    }

    // see: cans/05-storage-plane.md#Host-inode-lock-broker
    pub fn set_lock_namespace(&self, namespace: [u8; 16]) -> Result<(), PluginError> {
        self.coordination.set_lock_namespace(namespace)
    }

    async fn prepare_participants(&self) -> Result<(), PluginError> {
        // TODO (L3): issue SQL pre-commit on the coordinated SqlProxyService connections; participant failure here triggers global rollback with EPOLICY.
        let _ = self.peer_sql_proxy()?;
        // TODO (L3): pin advisory-lock acquisition to the declared physical host inodes via the coordinated lock broker.
        let _ = self.peer_lock_broker()?;
        Ok(())
    }

    async fn release_participant_locks(&self) -> Result<(), PluginError> {
        match self.peer_lock_broker()? {
            Some(locks) => match self.coordination.lock_namespace()? {
                Some(namespace) => locks.release_transaction_locks_for(namespace).await,
                None => locks.release_transaction_locks().await,
            },
            None => Ok(()),
        }
    }
}

impl KelirPlugin for TransactionPlugin {
    // see: cans/plugins/transaction.md#Service-exports-(transaction)
    type Context = Arc<dyn TransactionService>;
    fn context(&self) -> Self::Context {
        Arc::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "transaction"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
    // see: cans/plugins/transaction.md#Service-exports-(transaction)
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        let registry = pipeline.service_registry();
        registry.register::<dyn TransactionService>(Arc::new(self.clone()));
        // TODO (L3): Session::boot calls resolve_peers after every plugin is mounted so a peer mounted after this plugin is re-resolved.
        let _ = self.resolve_peers(registry);
    }
}

#[async_trait]
impl LifecycleParticipant for TransactionPlugin {
    // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
    async fn on_pre_commit(&self, ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        if self.config.flush_fuse_before_commit {
            // see: cans/08-lifecycle.md#Hypervisor-primitives
            ctx.freeze_vcpus().await?;
            ctx.drain_fuse(Duration::from_millis(self.config.timeout_ms))
                .await?;
        }
        self.prepare_participants().await?;
        Ok(())
    }

    // see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
    async fn on_post_commit(
        &self,
        ctx: &mut LifecycleCtx,
        committed: bool,
    ) -> Result<(), PluginError> {
        if committed {
            ctx.commit_broker_upper().await?;
            // TODO (L3): issue SQL COMMIT across active SqlProxyService proxy connections.
            let _ = self.peer_sql_proxy()?;
        } else {
            ctx.discard_broker_upper().await?;
            // TODO (L3): issue SQL ROLLBACK across active SqlProxyService proxy connections.
            let _ = self.peer_sql_proxy()?;
            self.release_participant_locks().await?;
        }
        if self.config.flush_fuse_before_commit {
            ctx.unfreeze_vcpus().await?;
        }
        Ok(())
    }
}
