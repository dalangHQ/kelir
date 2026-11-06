// see: cans/plugins/container.md
#![forbid(unsafe_code)]

use async_trait::async_trait;
use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{KelirPlugin, LifecycleCtx, LifecycleParticipant, MountScope};

pub mod service;

#[derive(Clone)]
pub struct ContainerPlugin;

impl ContainerPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ContainerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for ContainerPlugin {
    type Context = ();
    fn context(&self) {}
    fn name(&self) -> &'static str {
        "container"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
}

#[async_trait]
impl LifecycleParticipant for ContainerPlugin {
    async fn on_vm_ready(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        // TODO: Configure rootless Podman/crun cgroup v2 hierarchy
        Ok(())
    }
}
