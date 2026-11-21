use kelir::hlist_sdk::{HList, NoParams};
use kelir::interface::handlers_plugin::Capability;
use kelir::interface::plugin_model::PluginError;
use kelir::session_lifecycle::Session;
use plugin_ledger::LedgerCtx;

// see: cans/plugins/ledger.md#Canonical-accessor-(SSOT-A1-pin)
pub trait LedgerSessionExt: Sized {
    // see: cans/plugins/ledger.md#Causal-delta-access-(ledger)
    async fn get_causal_delta_from_parent(
        &self,
    ) -> Result<kelir::interface::telemetry::CausalDelta, PluginError>;

    // see: cans/plugins/ledger.md#Ledger-handler-API
    async fn count_denials(&self, ppid: u32) -> Result<u64, PluginError>;
}

// see: cans/plugins/ledger.md#Canonical-accessor-(SSOT-A1-pin)
impl<Plugins, Params> LedgerSessionExt for Session<Plugins, Params>
where
    Plugins: HList + Capability<LedgerCtx>,
{
    // see: cans/plugins/ledger.md#Causal-delta-access-(ledger)
    async fn get_causal_delta_from_parent(
        &self,
    ) -> Result<kelir::interface::telemetry::CausalDelta, PluginError> {
        let ledger = self.cap::<LedgerCtx>().ok_or_else(|| {
            PluginError::Halt("ECONFIG: ledger capability not mounted".into())
        })?;
        ledger.get_causal_delta_from_parent().await
    }

    // see: cans/plugins/ledger.md#Ledger-handler-API
    async fn count_denials(&self, ppid: u32) -> Result<u64, PluginError> {
        let ledger = self.cap::<LedgerCtx>().ok_or_else(|| {
            PluginError::Halt("ECONFIG: ledger capability not mounted".into())
        })?;
        ledger.count_denials(ppid).await
    }
}
