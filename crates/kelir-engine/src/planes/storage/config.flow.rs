// see: cans/05-storage-plane.md#Workspace-commit
use std::path::PathBuf;

// see: cans/05-storage-plane.md#Workspace-commit
pub const DEFAULT_BLOCKING_WAIT_TIMEOUT_MS: u64 = 5000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockBrokerWiring {
    pub scope_globs: Vec<String>,
    pub non_blocking: bool,
    // see: cans/05-storage-plane.md#Workspace-commit
    pub blocking_wait_timeout_ms: u64,
}

impl Default for LockBrokerWiring {
    fn default() -> Self {
        Self {
            scope_globs: Vec::new(),
            non_blocking: false,
            blocking_wait_timeout_ms: DEFAULT_BLOCKING_WAIT_TIMEOUT_MS,
        }
    }
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
    // see: cans/05-storage-plane.md#Workspace-commit
    pub default_permissions: bool,
}
