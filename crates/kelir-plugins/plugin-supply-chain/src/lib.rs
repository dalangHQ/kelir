// see: cans/plugins/supply-chain.md
#![forbid(unsafe_code)]

use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{HandlerCtx, KelirPlugin, MountScope, TrapInterposer};
use kelir_interface::protocol::{TrapEvent, Verdict};

#[derive(Clone)]
pub struct JitValidationService;

#[derive(Clone)]
pub struct SupplyChainPlugin {
    pub jit_service: JitValidationService,
}

impl SupplyChainPlugin {
    pub fn new() -> Self {
        Self {
            jit_service: JitValidationService,
        }
    }
}

impl Default for SupplyChainPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for SupplyChainPlugin {
    fn name(&self) -> &'static str {
        "supply-chain"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
}

impl TrapInterposer for SupplyChainPlugin {
    fn on_trap(&self, _event: &TrapEvent, _ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        // TODO: Inspect argv entropy (>5.2 bits) and block lifecycle scripts
        Ok(Verdict::Pass)
    }
}
