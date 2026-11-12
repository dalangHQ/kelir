// see: cans/plugins/transaction.md#Plugin-transaction
// see: cans/artifacts/code/transaction-service.rs#TransactionService
use std::path::PathBuf;

use async_trait::async_trait;
use kelir_interface::errno_types::Errno;
use kelir_interface::plugin_model::PluginError;

use crate::TransactionCtx;
use crate::savepoint_marker;

#[derive(Debug, Clone, Default)]
pub struct TransactionScope {
    pub paths: Vec<PathBuf>,
    pub schemas: Vec<String>,
    pub inodes: Vec<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransactionHandle {
    pub id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Savepoint {
    pub id: u64,
}

// see: cans/plugins/transaction.md#Service-exports-(transaction)
#[async_trait]
pub trait TransactionService: Send + Sync {
    async fn begin_scope(
        &mut self,
        scope: TransactionScope,
    ) -> Result<TransactionHandle, PluginError>;
    async fn commit(&mut self, handle: TransactionHandle) -> Result<(), PluginError>;
    async fn abort(&mut self, handle: TransactionHandle) -> Result<(), PluginError>;
    async fn savepoint(&mut self, handle: &TransactionHandle) -> Result<Savepoint, PluginError>;
    async fn rollback_to_savepoint(
        &mut self,
        handle: &TransactionHandle,
        savepoint: Savepoint,
    ) -> Result<(), PluginError>;
}

// see: cans/plugins/transaction.md#Service-exports-(transaction)
#[async_trait]
impl TransactionService for TransactionCtx {
    async fn begin_scope(
        &mut self,
        scope: TransactionScope,
    ) -> Result<TransactionHandle, PluginError> {
        self.coordination().begin_scope(scope)
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    async fn commit(&mut self, handle: TransactionHandle) -> Result<(), PluginError> {
        let coordination = self.coordination();
        coordination.require_active(&handle)?;
        if let Err(err) = coordination.prepare_participants().await {
            coordination.abort_active().await?;
            return Err(PluginError::reject(
                Errno::Epolicy,
                format!("2PC prepare failed; participants rolled back: {err}"),
            ));
        }
        // TODO (L3): broker upper replay rides the on_post_commit(true) driver hook per cans/plugins/transaction.md#Multi-resource-2PC-coordination.
        if let Err(err) = coordination.issue_sql_commit().await {
            coordination.abort_active().await?;
            return Err(err);
        }
        coordination.release_transaction_locks_scoped().await?;
        coordination.clear_active()
    }

    async fn abort(&mut self, handle: TransactionHandle) -> Result<(), PluginError> {
        // TODO (L3): un-rollbackable L7 egress during the transaction aborts with E_TRANSACTION_TAINT before participant cleanup.
        self.coordination().require_active(&handle)?;
        self.coordination().abort_active().await
    }

    // see: cans/plugins/transaction.md#Temporal-savepoints
    async fn savepoint(&mut self, handle: &TransactionHandle) -> Result<Savepoint, PluginError> {
        self.coordination().require_active(handle)?;
        let savepoint = Savepoint {
            id: self.coordination().next_savepoint_id(),
        };
        let marker = savepoint_marker(savepoint);
        self.coordination().record_savepoint(savepoint, *handle)?;
        self.coordination().inject_sql_savepoint(&marker).await?;
        Ok(savepoint)
    }

    // see: cans/plugins/transaction.md#Temporal-savepoints
    async fn rollback_to_savepoint(
        &mut self,
        handle: &TransactionHandle,
        savepoint: Savepoint,
    ) -> Result<(), PluginError> {
        self.coordination().require_active(handle)?;
        self.coordination()
            .rollback_recorded_savepoint(handle, savepoint)
    }
}
