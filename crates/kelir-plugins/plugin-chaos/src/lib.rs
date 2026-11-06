// see: cans/plugins/chaos.md
#![forbid(unsafe_code)]

use std::path::Path;

use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{FsInterposer, HandlerCtx, KelirPlugin, MountScope, NetInterposer};
use kelir_interface::protocol::FilterAction;

#[derive(Clone)]
pub struct FaultInjectionService;

#[derive(Clone)]
pub struct ChaosPlugin {
    pub fault_service: FaultInjectionService,
}

impl ChaosPlugin {
    pub fn new() -> Self {
        Self {
            fault_service: FaultInjectionService,
        }
    }
}

impl Default for ChaosPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for ChaosPlugin {
    type Context = ();
    fn context(&self) {}
    fn name(&self) -> &'static str {
        "chaos"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
}

impl NetInterposer for ChaosPlugin {
    fn on_net_outbound(&self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        // TODO: Inject synthetic network latency or drop frames
        Ok(FilterAction::Forward)
    }
}

impl FsInterposer for ChaosPlugin {
    fn on_fs_write(
        &self,
        _path: &Path,
        _data: &mut [u8],
        _pid: u32,
        _ctx: &HandlerCtx,
    ) -> Result<FilterAction, PluginError> {
        // TODO: Inject synthetic ENOSPC failures
        Ok(FilterAction::Forward)
    }
}
