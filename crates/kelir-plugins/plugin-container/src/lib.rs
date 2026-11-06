// see: cans/plugins/container.md
#![forbid(unsafe_code)]

use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{KelirPlugin, LifecycleCtx, LifecycleParticipant, MountScope};

pub mod service;

#[derive(Clone)]
pub struct ContainerService;

#[derive(Clone)]
pub struct ContainerPlugin {
    pub service: ContainerService,
}

impl ContainerPlugin {
    pub fn new() -> Self {
        Self {
            service: ContainerService,
        }
    }
}

impl Default for ContainerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for ContainerPlugin {
    fn name(&self) -> &'static str {
        "container"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
}

impl LifecycleParticipant for ContainerPlugin {
    fn on_vm_ready(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        // TODO: Configure rootless Podman/crun cgroup v2 hierarchy
        Ok(())
    }
}
