// see: cans/05-storage-plane.md#Workspace-commit
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LockBrokerWiring {
    pub scope_globs: Vec<String>,
    pub non_blocking: bool,
}

// see: cans/05-storage-plane.md#Virtual-ID-remapping
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdmapConfig {
    pub guest_uid: u32,
    pub host_uid: u32,
}

// see: cans/05-storage-plane.md#In-flight-read-masking
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskReadRule {
    pub glob: String,
    pub patterns: Vec<(String, String)>,
}

// see: cans/05-storage-plane.md#In-flight-read-masking
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReadMaskEngine {
    #[default]
    FastRegex,
}

// see: cans/05-storage-plane.md#In-flight-read-masking
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MaskReadConfig {
    pub engine: ReadMaskEngine,
    pub carryover_bytes: usize,
    pub rules: Vec<MaskReadRule>,
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
    pub mask_read: Option<MaskReadConfig>,
    pub encapsulation: WorkspaceEncapsulation,
}
