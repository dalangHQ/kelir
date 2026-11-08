// see: cans/plugins/transaction.md
// see: cans/artifacts/code/transaction-service.rs
use std::path::PathBuf;

use async_trait::async_trait;
use kelir_interface::plugin::PluginError;

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

// see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
#[async_trait]
impl TransactionService for crate::TransactionPlugin {
    async fn begin_scope(
        &mut self,
        scope: TransactionScope,
    ) -> Result<TransactionHandle, PluginError> {
        // TODO (L3): open the scope over the declared blast radius; out-of-scope mutations reject with EACCES or EAmbiguousMutation.
        let _ = scope;
        Ok(TransactionHandle { id: 0 })
    }
    async fn commit(&mut self, _handle: TransactionHandle) -> Result<(), PluginError> {
        // TODO (L3): broker commit (replay upper + tombstones) then SQL COMMIT across active proxy connections.
        Ok(())
    }
    async fn abort(&mut self, _handle: TransactionHandle) -> Result<(), PluginError> {
        // TODO (L3): discard broker upper and tombstones, issue SQL ROLLBACK, release advisory locks; deadline expiry aborts with ETIMEDOUT.
        Ok(())
    }
    async fn savepoint(&mut self, _handle: &TransactionHandle) -> Result<Savepoint, PluginError> {
        // TODO (L3): inject SQL SAVEPOINT markers into active pgwire connections and snapshot broker upper and tombstone state.
        Ok(Savepoint { id: 0 })
    }
    async fn rollback_to_savepoint(
        &mut self,
        _handle: &TransactionHandle,
        _savepoint: Savepoint,
    ) -> Result<(), PluginError> {
        // TODO (L3): restore intermediate savepoint state without aborting the parent transaction.
        Ok(())
    }
}
