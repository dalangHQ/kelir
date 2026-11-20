// see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
use async_trait::async_trait;
use kelir_interface::plugin_model::{LifecycleCtx, LifecycleParticipant, PluginError};

use crate::coordination::TransactionPlugin;

// see: cans/plugins/transaction.md#Invariants-and-policy-(transaction)
#[async_trait]
impl LifecycleParticipant for TransactionPlugin {
    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    async fn on_pre_commit(&self, _ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        // TODO (L3) [relational_transaction]: freeze vCPUs, drain FUSE, and prepare 2PC participants; stub does nothing.; slop: vCPUs never freeze and FUSE never drains, 2PC participants are never prepared, a commit runs against live state.
        Err(PluginError::Halt(
            "TODO (L3): freeze vCPUs, drain FUSE, and prepare 2PC participants".into(),
        ))
    }

    // see: cans/plugins/transaction.md#Multi-resource-2PC-coordination
    async fn on_post_commit(
        &self,
        _ctx: &mut LifecycleCtx<'_>,
        _committed: bool,
    ) -> Result<(), PluginError> {
        // TODO (L3) [relational_transaction]: commit or abort the broker upper, SQL, and active scope, then unfreeze vCPUs; stub does nothing.; slop: commit or abort never runs, the broker upper, SQL, and scope diverge, vCPUs stay frozen after the transaction.
        Err(PluginError::Halt(
            "TODO (L3): commit or abort the broker upper, SQL, and active scope, then unfreeze vCPUs".into(),
        ))
    }
}
