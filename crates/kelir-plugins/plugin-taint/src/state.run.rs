// see: cans/plugins/taint.md#Plugin-taint
use std::path::Path;

use async_trait::async_trait;
use kelir_interface::plugin_model::{
    FsInterposer, HandlerCtx, NetInterposer, PluginError, TrapInterposer,
};
use kelir_interface::protocol_wire::{FilterAction, TrapEvent, Verdict};

use crate::state::TaintPlugin;

// see: cans/02-plugin-model.md#KelirPlugin-trait
#[async_trait]
impl TrapInterposer for TaintPlugin {
    async fn on_trap(&self, _event: &TrapEvent, _ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        // TODO (L3) [governance_tools]: propagate FUSE-read taint labels into execve argv/envp spans; slop: tainted reads execute unflagged; argv/envp carry no taint labels.
        Ok(Verdict::Pass)
    }
}

impl FsInterposer for TaintPlugin {
    fn on_fs_read(&self, _path: &Path, buffer: &[u8]) -> Result<Vec<u8>, PluginError> {
        // TODO (L3) [governance_tools]: label the read span when the source carries untrusted ingress; slop: reads from untrusted ingress run unlabeled; downstream spans miss the taint.
        Ok(buffer.to_vec())
    }

    fn on_fs_write(
        &self,
        _path: &Path,
        _data: &mut [u8],
        _pid: u32,
        _ctx: &HandlerCtx,
    ) -> Result<FilterAction, PluginError> {
        // TODO (L3) [governance_tools]: propagate taint labels into written bytes before the verdict; slop: tainted writes reach the verdict unlabeled; the gate cannot see the taint.
        Ok(FilterAction::Forward)
    }
}

impl NetInterposer for TaintPlugin {
    fn on_net_outbound(&self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        // TODO (L3) [governance_tools]: hold egress spans that overlap a taint label for escalation; slop: tainted egress leaves without escalation; exfiltration spans pass unheld.
        Ok(FilterAction::Forward)
    }
}
