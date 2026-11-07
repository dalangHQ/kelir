// see: cans/plugins/transaction.md
#![forbid(unsafe_code)]

// TODO (L3): tests/test_transaction.rs per cans/artifacts/config/project-structure.txt.
use async_trait::async_trait;
use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{KelirPlugin, LifecycleCtx, LifecycleParticipant, MountScope};

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
    async fn on_pre_commit(&self, ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        let _ = ctx;
        // TODO (L3): vCPU freeze, FUSE drain, and broker handles are unreachable here; LifecycleCtx exposes watchdog, balloon, vetoes only per cans/02#Hook-context-restriction.
        Ok(())
    }
    async fn on_post_commit(
        &self,
        _ctx: &mut LifecycleCtx,
        _committed: bool,
    ) -> Result<(), PluginError> {
        // TODO (L3): on true commit broker upper plus SQL COMMIT, on false discard upper plus SQL ROLLBACK; both need broker and proxy handles LifecycleCtx cannot supply.
        Ok(())
    }
}
