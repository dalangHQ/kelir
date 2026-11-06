// see: cans/plugins/transaction.md
#![forbid(unsafe_code)]

// TODO (L3): tests/test_transaction.rs per cans/artifacts/config/project-structure.txt.
use async_trait::async_trait;
use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{KelirPlugin, LifecycleParticipant, MountScope};

pub mod service;

#[derive(Clone)]
pub struct TransactionPlugin;

impl TransactionPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TransactionPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for TransactionPlugin {
    type Context = ();
    fn context(&self) {}
    fn name(&self) -> &'static str {
        "transaction"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
}

#[async_trait]
impl LifecycleParticipant for TransactionPlugin {
    async fn on_pre_commit(&self) -> Result<(), PluginError> {
        // TODO: Coordinate 2PC pre-commit with db-proxy
        Ok(())
    }
    async fn on_post_commit(&self, _success: bool) -> Result<(), PluginError> {
        Ok(())
    }
}
