// see: cans/plugins/transaction.md#Plugin-transaction
// see: cans/artifacts/code/transaction-service.rs#TransactionService
use std::path::PathBuf;

use async_trait::async_trait;
use kelir_interface::plugin::PluginError;

use crate::TransactionCtx;

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

    async fn commit(&mut self, handle: TransactionHandle) -> Result<(), PluginError> {
        // TODO (L3): trigger the session two-phase commit path so on_pre_commit and on_post_commit drive broker and SQL participants in order.
        self.coordination().require_active(&handle)?;
        Ok(())
    }

    async fn abort(&mut self, handle: TransactionHandle) -> Result<(), PluginError> {
        // TODO (L3): un-rollbackable L7 egress during the transaction aborts with E_TRANSACTION_TAINT before participant cleanup.
        self.coordination().require_active(&handle)?;
        self.coordination().abort_active().await
    }

    async fn savepoint(&mut self, handle: &TransactionHandle) -> Result<Savepoint, PluginError> {
        self.coordination().require_active(handle)?;
        let savepoint = Savepoint {
            id: self.coordination().next_savepoint_id(),
        };
        // TODO (L3): snapshot the broker upper and tombstone state as an opaque savepoint blob.
        self.coordination()
            .inject_sql_savepoint(&format!("kelir_sp_{}", savepoint.id))
            .await?;
        Ok(savepoint)
    }

    async fn rollback_to_savepoint(
        &mut self,
        handle: &TransactionHandle,
        savepoint: Savepoint,
    ) -> Result<(), PluginError> {
        self.coordination().require_active(handle)?;
        // TODO (L3): issue SQL ROLLBACK TO the savepoint marker and restore the snapshotted broker upper and tombstone state.
        let _ = savepoint;
        Ok(())
    }
}
