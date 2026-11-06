// see: cans/plugins/wire-ingress.md
#![forbid(unsafe_code)]

// TODO (L3): tests/test_parking.rs per cans/artifacts/config/project-structure.txt.
use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{KelirPlugin, LifecycleParticipant, MountScope, NetInterposer};
use kelir_interface::protocol::FilterAction;

#[derive(Clone)]
pub struct IngressRoutingService;

#[derive(Clone)]
pub struct WireIngressPlugin {
    pub router: IngressRoutingService,
}

impl WireIngressPlugin {
    pub fn new() -> Self {
        Self {
            router: IngressRoutingService,
        }
    }
}

impl Default for WireIngressPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for WireIngressPlugin {
    fn name(&self) -> &'static str {
        "wire-ingress"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
}

impl LifecycleParticipant for WireIngressPlugin {}
impl NetInterposer for WireIngressPlugin {
    fn on_net_inbound(&self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        // TODO: Park inbound connections during VM suspend
        Ok(FilterAction::Forward)
    }
}
