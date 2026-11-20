// see: cans/plugins/container.md#Plugin-container
#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::sync::Arc;

use kelir_interface::plugin_model::PluginDeps;

#[path = "service_container.flow.rs"]
pub mod service_container;
#[path = "service_container.run.rs"]
pub mod service_container_run;

pub use service_container::{
    ContainerCtx, ContainerHandle, ContainerService, SharedContainerService,
};

// see: cans/plugins/container.md#UID/GID-mapping-isolation
const NEWUIDMAP: &str = "newuidmap";

// see: cans/plugins/container.md#UID/GID-mapping-isolation
const NEWGIDMAP: &str = "newgidmap";

// see: cans/plugins/container.md#Non-nested-OCI-container-execution
const PODMAN: &str = "podman";

// see: cans/05-storage-plane.md#Host-inode-lock-broker
const SUBUID_PATH: &str = "/etc/subuid";

// see: cans/05-storage-plane.md#Host-inode-lock-broker
const SUBGID_PATH: &str = "/etc/subgid";

// see: cans/plugins/container.md#UID/GID-mapping-isolation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdMapKind {
    Uid,
    Gid,
}

// see: cans/plugins/container.md#UID/GID-mapping-isolation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdMapAllocation {
    pub lower: u32,
    pub upper: u32,
    pub count: u32,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.container
// see: cans/plugins/profile.md#Default-zero-capability-invariant
#[derive(Debug, Clone)]
pub struct ContainerConfig {
    pub runtime: String,
    pub isolate_var_lib_containers: bool,
    pub allow_unsandboxed: bool,
    pub pin_subuid_ranges: bool,
    pub scratch_device: Option<PathBuf>,
}

// see: cans/plugins/container.md#Plugin-container
#[derive(Clone, PluginDeps)]
pub struct ContainerPlugin {
    config: ContainerConfig,
    service: Arc<SharedContainerService>,
}
