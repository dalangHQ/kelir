// see: cans/plugins/mesh.md
#![forbid(unsafe_code)]

// see: cans/artifacts/config/project-structure.txt
use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{KelirPlugin, LifecycleParticipant, MountScope, NetInterposer};
use kelir_interface::protocol::FilterAction;

#[derive(Clone)]
pub struct MeshPlugin;

impl MeshPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MeshPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for MeshPlugin {
    fn name(&self) -> &'static str {
        "mesh"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
}

impl LifecycleParticipant for MeshPlugin {}
impl NetInterposer for MeshPlugin {
    fn on_net_outbound(&self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        // see: cans/plugins/mesh.md
        Ok(FilterAction::Forward)
    }
    fn on_net_inbound(&self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        Ok(FilterAction::Forward)
    }
}
