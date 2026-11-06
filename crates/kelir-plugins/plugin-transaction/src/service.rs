// see: cans/plugins/transaction.md
use async_trait::async_trait;
use kelir_interface::errors::PluginError;
use std::path::PathBuf;

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

#[async_trait]
impl TransactionService for crate::TransactionPlugin {
    async fn begin_scope(
        &mut self,
        _scope: TransactionScope,
    ) -> Result<TransactionHandle, PluginError> {
        // see: cans/plugins/transaction.md
        Ok(TransactionHandle { id: 0 })
    }
    async fn commit(&mut self, _handle: TransactionHandle) -> Result<(), PluginError> {
        // see: cans/plugins/transaction.md
        Ok(())
    }
    async fn abort(&mut self, _handle: TransactionHandle) -> Result<(), PluginError> {
        // see: cans/plugins/transaction.md
        Ok(())
    }
    async fn savepoint(&mut self, _handle: &TransactionHandle) -> Result<Savepoint, PluginError> {
        // see: cans/plugins/transaction.md
        Ok(Savepoint { id: 0 })
    }
    async fn rollback_to_savepoint(
        &mut self,
        _handle: &TransactionHandle,
        _savepoint: Savepoint,
    ) -> Result<(), PluginError> {
        // see: cans/plugins/transaction.md
        Ok(())
    }
}
