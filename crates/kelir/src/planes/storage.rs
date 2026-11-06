use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use kelir_interface::errors::PluginError;

use crate::HandlerCtx;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdmapConfig {
    pub guest_uid: u32,
    pub host_uid: u32,
}

#[derive(Debug, Clone)]
pub struct DynamicFile {
    pub content: String,
    pub mode: u32,
}

pub type DynamicFileGenerator = Arc<
    dyn Fn(&Path) -> Pin<Box<dyn Future<Output = Result<DynamicFile, PluginError>> + Send>>
        + Send
        + Sync,
>;

#[derive(Clone)]
pub enum SyntheticFile {
    Static {
        path: String,
        content: String,
    },
    Dynamic {
        path: String,
        generator: DynamicFileGenerator,
    },
}

impl SyntheticFile {
    pub fn static_(path: &str, content: &str) -> Self {
        Self::Static {
            path: path.to_string(),
            content: content.to_string(),
        }
    }

    pub fn dynamic<F>(path: &str, f: F) -> Self
    where
        F: Fn(&Path) -> Pin<Box<dyn Future<Output = Result<DynamicFile, PluginError>> + Send>>
            + Send
            + Sync
            + 'static,
    {
        Self::Dynamic {
            path: path.to_string(),
            generator: Arc::new(f),
        }
    }
}

impl std::fmt::Debug for SyntheticFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Static { path, .. } => f
                .debug_struct("SyntheticFile::Static")
                .field("path", path)
                .finish_non_exhaustive(),
            Self::Dynamic { path, .. } => f
                .debug_struct("SyntheticFile::Dynamic")
                .field("path", path)
                .finish_non_exhaustive(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MaskRule {
    pub glob: String,
    pub patterns: Vec<(String, String)>,
}

pub type SnoopHandler = Arc<
    dyn Fn(
            PathBuf,
            Vec<u8>,
            HandlerCtx,
        ) -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send>>
        + Send
        + Sync,
>;

#[derive(Clone)]
pub struct SnoopRule {
    pub glob: String,
    pub sync: bool,
    pub on_write: SnoopHandler,
}

impl std::fmt::Debug for SnoopRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SnoopRule")
            .field("glob", &self.glob)
            .field("sync", &self.sync)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceEncapsulation {
    Passthrough,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadMaskEngine {
    FastRegex,
}

#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub idmap: IdmapConfig,
    pub shared_caches: Vec<String>,
    pub lock_broker: bool,
    pub synthetic_files: Vec<SyntheticFile>,
    pub mask_read: Vec<MaskRule>,
    pub snoops: Vec<SnoopRule>,
    pub encapsulation: WorkspaceEncapsulation,
    pub scan_engine: ReadMaskEngine,
    pub carryover_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct FsReconcileRequest {
    pub cwd: String,
    pub atomic_staging: bool,
    pub fsync: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconcileStats {
    pub files_changed: usize,
    pub tombstones_resolved: usize,
}

#[derive(Debug, Clone)]
pub struct LockBrokerConfig {
    pub scope: LockScope,
    pub default_mode: LockMode,
    pub non_blocking: bool,
}

#[derive(Debug, Clone)]
pub enum LockScope {
    Globs(Vec<String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockMode {
    Exclusive,
}
