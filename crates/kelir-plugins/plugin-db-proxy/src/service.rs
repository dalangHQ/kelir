// see: cans/plugins/db-proxy.md#Plugin-db-proxy
// see: cans/plugins/transaction.md#Crate-dependencies-(transaction)
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use async_trait::async_trait;
use kelir_interface::plugin::PluginError;

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlDialect {
    Postgres,
    Sqlite,
}

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StatementHandle {
    pub id: u64,
}

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
#[async_trait]
pub trait SqlProxyService: Send + Sync {
    async fn dialect(&self) -> Result<SqlDialect, PluginError>;
    async fn register_statement(&self, sql: &str) -> Result<StatementHandle, PluginError>;
    async fn lookup_statement(&self, handle: StatementHandle) -> Result<String, PluginError>;
    async fn issue_commit(&self) -> Result<(), PluginError>;
    async fn issue_rollback(&self) -> Result<(), PluginError>;
    async fn inject_savepoint(&self, marker: &str) -> Result<(), PluginError>;
}

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
struct StatementTable {
    next_id: u64,
    statements: HashMap<u64, String>,
}

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
pub struct SqlProxy {
    dialect: SqlDialect,
    statements: Mutex<StatementTable>,
    connections: Mutex<HashSet<u32>>,
}

impl SqlProxy {
    pub fn new(dialect: SqlDialect) -> Self {
        Self {
            dialect,
            statements: Mutex::new(StatementTable {
                next_id: 0,
                statements: HashMap::new(),
            }),
            connections: Mutex::new(HashSet::new()),
        }
    }

    pub fn track_connection(&self, stream_id: u32) -> Result<(), PluginError> {
        let mut connections = self
            .connections
            .lock()
            .map_err(|_| PluginError::Halt("proxy connection table poisoned".into()))?;
        connections.insert(stream_id);
        Ok(())
    }

    pub fn release_connection(&self, stream_id: u32) {
        if let Ok(mut connections) = self.connections.lock() {
            connections.remove(&stream_id);
        }
    }
}

#[async_trait]
impl SqlProxyService for SqlProxy {
    async fn dialect(&self) -> Result<SqlDialect, PluginError> {
        Ok(self.dialect)
    }

    async fn register_statement(&self, sql: &str) -> Result<StatementHandle, PluginError> {
        let mut table = self
            .statements
            .lock()
            .map_err(|_| PluginError::Halt("statement table poisoned".into()))?;
        table.next_id = table.next_id.saturating_add(1);
        let id = table.next_id;
        table.statements.insert(id, sql.into());
        Ok(StatementHandle { id })
    }

    async fn lookup_statement(&self, handle: StatementHandle) -> Result<String, PluginError> {
        let table = self
            .statements
            .lock()
            .map_err(|_| PluginError::Halt("statement table poisoned".into()))?;
        match table.statements.get(&handle.id) {
            Some(sql) => Ok(sql.clone()),
            None => Err(PluginError::reject(
                "ENOENT",
                "statement handle is not registered",
            )),
        }
    }

    async fn issue_commit(&self) -> Result<(), PluginError> {
        // TODO (L3): issue SQL COMMIT across every active proxy connection per cans/plugins/transaction.md.
        Ok(())
    }

    async fn issue_rollback(&self) -> Result<(), PluginError> {
        // TODO (L3): issue SQL ROLLBACK across every active proxy connection per cans/plugins/transaction.md.
        Ok(())
    }

    async fn inject_savepoint(&self, marker: &str) -> Result<(), PluginError> {
        let _ = marker;
        // TODO (L3): inject a transparent SQL SAVEPOINT marker into active pgwire connections per cans/plugins/transaction.md.
        Ok(())
    }
}
