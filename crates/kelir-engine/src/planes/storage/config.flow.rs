// see: cans/05-storage-plane.md#Workspace-commit
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LockBrokerWiring {
    pub scope_globs: Vec<String>,
    pub non_blocking: bool,
}

// see: cans/05-storage-plane.md#Virtual-ID-remapping
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IdmapConfig {
    pub guest_uid: u32,
    pub host_uid: u32,
}

// see: cans/05-storage-plane.md#Storage-architecture
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WorkspaceEncapsulation {
    #[default]
    Locked,
    Passthrough,
}

// see: cans/05-storage-plane.md#Storage-architecture
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StorageConfig {
    pub shared_caches: Vec<PathBuf>,
    pub lock_broker: Option<LockBrokerWiring>,
    pub idmap: Option<IdmapConfig>,
    pub encapsulation: WorkspaceEncapsulation,
}
