// see: cans/plugins/transaction.md
#![forbid(unsafe_code)]

use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{KelirPlugin, LifecycleParticipant, MountScope};

pub mod service;

#[derive(Clone)]
pub struct TransactionService;

#[derive(Clone)]
pub struct TransactionPlugin {
    pub service: TransactionService,
}

impl TransactionPlugin {
    pub fn new() -> Self {
        Self {
            service: TransactionService,
        }
    }
}

impl Default for TransactionPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for TransactionPlugin {
    fn name(&self) -> &'static str {
        "transaction"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
}

impl LifecycleParticipant for TransactionPlugin {
    fn on_pre_commit(&self) -> Result<(), PluginError> {
        // TODO: Coordinate 2PC pre-commit with db-proxy
        Ok(())
    }
    fn on_post_commit(&self, _success: bool) -> Result<(), PluginError> {
        Ok(())
    }
}
