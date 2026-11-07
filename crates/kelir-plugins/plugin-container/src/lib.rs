// see: cans/plugins/container.md
#![forbid(unsafe_code)]

use std::sync::Arc;

use async_trait::async_trait;
use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{
    HandlerCtx, KelirPlugin, LifecycleCtx, LifecycleParticipant, MountScope, PipelineBinder,
    TrapInterposer, VirtioDevice,
};
use kelir_interface::protocol::{TrapEvent, Verdict};

pub mod service;

#[derive(Clone)]
pub struct ContainerPlugin;

impl ContainerPlugin {
    pub fn new() -> Self {
        Self
    }

    pub fn is_id_map_binary(binary_path: &str) -> bool {
        let base = binary_path.rsplit('/').next().unwrap_or("");
        base == "newuidmap" || base == "newgidmap"
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
        MountScope::default().with_devices([VirtioDevice::Fs])
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        pipeline.bind_trap(Arc::new(self.clone()));
    }
}

#[async_trait]
impl LifecycleParticipant for ContainerPlugin {
    async fn on_vm_ready(&self, _ctx: &mut LifecycleCtx) -> Result<(), PluginError> {
        // TODO (L3): configure rootless Podman/crun cgroup v2 hierarchy.
        Ok(())
    }
}

#[async_trait]
impl TrapInterposer for ContainerPlugin {
    async fn on_trap(&self, event: &TrapEvent, _ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        if !Self::is_id_map_binary(&event.binary_path) {
            return Ok(Verdict::Pass);
        }
        let mounts_raw_device = event.argv.iter().any(|arg| arg.starts_with("/dev/"));
        if mounts_raw_device {
            return Ok(Verdict::Drop {
                errno: Some("EPERM".to_string()),
                reason: "container: raw device mount rejected".to_string(),
            });
        }
        // TODO (L3): pin the argv subuid/subgid ranges to the unprivileged guest ranges via ContainerService::pin_id_map; range enforcement is open in the artifact.
        Ok(Verdict::Pass)
    }
}
