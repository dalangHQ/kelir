// see: cans/plugins/transaction.md#Plugin-transaction
// see: cans/artifacts/code/transaction-service.rs#TransactionService
use std::path::PathBuf;

use async_trait::async_trait;
use kelir_interface::plugin_model::PluginError;

use crate::TransactionCtx;
use crate::coordination::savepoint_marker;

#[derive(Debug, Clone, Default)]
pub struct TransactionScope {
    pub paths: Vec<PathBuf>,
    pub schemas: Vec<String>,
    pub inodes: Vec<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransactionScopeHandle {
    pub id: u64,
}

// see: cans/plugins/transaction.md#Scope-pin-(SSOT-A1)
pub type TransactionHandle = TransactionCtx;

// see: cans/plugins/transaction.md#Scope-pin-(SSOT-A1)
#[derive(Clone)]
pub struct Tx {
    pub name: String,
    ctx: TransactionCtx,
    handle: TransactionScopeHandle,
}

impl Tx {
    // see: cans/plugins/transaction.md#Temporal-savepoints
    pub async fn savepoint(&mut self) -> Result<Savepoint, PluginError> {
        self.ctx.savepoint(&self.handle).await
    }

    // see: cans/plugins/transaction.md#Temporal-savepoints
    pub async fn rollback_to_savepoint(&mut self, savepoint: Savepoint) -> Result<(), PluginError> {
        self.ctx
            .rollback_to_savepoint(&self.handle, savepoint)
            .await
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    pub async fn commit(&mut self) -> Result<(), PluginError> {
        self.ctx.commit(self.handle).await
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    pub async fn abort(&mut self) -> Result<(), PluginError> {
        self.ctx.abort(self.handle).await
    }
}

impl TransactionCtx {
    // see: cans/plugins/transaction.md#Scope-pin-(SSOT-A1)
    pub async fn scope(
        &self,
        name: &str,
        paths: Vec<PathBuf>,
        schemas: Vec<String>,
    ) -> Result<Tx, PluginError> {
        let mut ctx = self.clone();
        let handle = ctx
            .begin_scope(TransactionScope {
                paths,
                schemas,
                inodes: Vec::new(),
            })
            .await?;
        Ok(Tx {
            name: name.to_string(),
            ctx,
            handle,
        })
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    pub fn buffer_side_effect(&self, len: usize) -> Result<u64, PluginError> {
        self.coordination().buffer_side_effect(len)
    }
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
    ) -> Result<TransactionScopeHandle, PluginError>;
    async fn commit(&mut self, handle: TransactionScopeHandle) -> Result<(), PluginError>;
    async fn abort(&mut self, handle: TransactionScopeHandle) -> Result<(), PluginError>;
    async fn savepoint(
        &mut self,
        handle: &TransactionScopeHandle,
    ) -> Result<Savepoint, PluginError>;
    async fn rollback_to_savepoint(
        &mut self,
        handle: &TransactionScopeHandle,
        savepoint: Savepoint,
    ) -> Result<(), PluginError>;
}

// see: cans/plugins/transaction.md#Service-exports-(transaction)
#[async_trait]
impl TransactionService for TransactionCtx {
    async fn begin_scope(
        &mut self,
        scope: TransactionScope,
    ) -> Result<TransactionScopeHandle, PluginError> {
        self.coordination().begin_scope(scope)
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    async fn commit(&mut self, handle: TransactionScopeHandle) -> Result<(), PluginError> {
        let coordination = self.coordination();
        coordination.require_active(&handle)?;
        if let Err(err) = coordination.prepare_participants().await {
            coordination.abort_active().await?;
            return Err(PluginError::Halt(format!(
                "EPOLICY: 2PC prepare failed; participants rolled back: {err}"
            )));
        }
        // L3-GAP: [relational_transaction]: broker upper replay rides the on_post_commit(true) driver hook per cans/plugins/transaction.md#Multi-resource-2PC-coordination; slop: broker replay never rides on_post_commit, post-commit diverges from DB
        if let Err(err) = coordination.issue_sql_commit().await {
            coordination.abort_active().await?;
            return Err(err);
        }
        coordination.release_outbox()?;
        coordination.release_transaction_locks_scoped().await?;
        coordination.clear_active()
    }

    async fn abort(&mut self, handle: TransactionScopeHandle) -> Result<(), PluginError> {
        // L3-GAP: [rt_08_side_effect_egress_transaction_taint]: un-rollbackable L7 egress during the transaction aborts with E_TRANSACTION_TAINT before participant cleanup; slop: egress allowed mid-transaction, webhook fires before commit, irreversible
        self.coordination().require_active(&handle)?;
        self.coordination().abort_active().await
    }

    // see: cans/plugins/transaction.md#Temporal-savepoints
    async fn savepoint(
        &mut self,
        handle: &TransactionScopeHandle,
    ) -> Result<Savepoint, PluginError> {
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
        handle: &TransactionScopeHandle,
        savepoint: Savepoint,
    ) -> Result<(), PluginError> {
        self.coordination().require_active(handle)?;
        self.coordination()
            .rollback_recorded_savepoint(handle, savepoint)
    }
}
