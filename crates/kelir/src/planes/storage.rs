use std::path::{Path, PathBuf};
use std::sync::Arc;

use kelir_interface::plugin::HandlerCtx;
use kelir_interface::plugin::PluginError;

use crate::hlist::{HCons, HList, HNil, NoParams};

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

// see: cans/05-storage-plane.md#Synthetic-FUSE-inodes
pub type DynamicFileGenerator =
    Arc<dyn Fn(&Path) -> Result<DynamicFile, PluginError> + Send + Sync>;

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

    // see: cans/05-storage-plane.md#Synthetic-FUSE-inodes
    pub fn dynamic<F>(path: &str, f: F) -> Self
    where
        F: Fn(&Path) -> Result<DynamicFile, PluginError> + Send + Sync + 'static,
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

// see: cans/02-plugin-model.md#Shape-of-the-surface
// see: cans/05-storage-plane.md#Interception-pipeline
pub type SnoopHandler<Plugins = HNil, Params = NoParams> = Arc<
    dyn Fn(PathBuf, Vec<u8>, HandlerCtx<Plugins, Params>) -> Result<(), PluginError> + Send + Sync,
>;

#[derive(Clone)]
pub struct SnoopRule<Plugins = HNil, Params = NoParams> {
    pub glob: String,
    pub sync: bool,
    pub on_write: SnoopHandler<Plugins, Params>,
}

impl<Plugins, Params> std::fmt::Debug for SnoopRule<Plugins, Params> {
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
pub struct StorageConfig<Plugins = HNil, Params = NoParams> {
    pub idmap: IdmapConfig,
    pub shared_caches: Vec<String>,
    pub lock_broker: bool,
    pub synthetic_files: Vec<SyntheticFile>,
    pub mask_read: Vec<MaskRule>,
    pub snoops: Vec<SnoopRule<Plugins, Params>>,
    pub encapsulation: WorkspaceEncapsulation,
    pub scan_engine: ReadMaskEngine,
    pub carryover_bytes: usize,
}

fn retag_snoop<Plugins, Params>(
    inner: SnoopHandler<Plugins, Params>,
) -> SnoopHandler<Plugins, NoParams>
where
    Plugins: HList,
    Params: 'static,
{
    Arc::new(
        move |path: PathBuf, raw: Vec<u8>, c: HandlerCtx<Plugins, NoParams>| {
            inner(path, raw, c.retag::<Params>())
        },
    )
}

fn narrow_snoop<Plugins, Head>(
    inner: SnoopHandler<Plugins, NoParams>,
) -> SnoopHandler<HCons<Head, Plugins>, NoParams>
where
    Plugins: HList,
    Head: Clone + Send + Sync + 'static,
{
    Arc::new(
        move |path: PathBuf, raw: Vec<u8>, c: HandlerCtx<HCons<Head, Plugins>, NoParams>| {
            inner(path, raw, c.map_plugins(|plugins| plugins.tail_value()))
        },
    )
}

impl<Plugins, Params> SnoopRule<Plugins, Params>
where
    Plugins: HList,
    Params: 'static,
{
    pub(crate) fn retag(self) -> SnoopRule<Plugins, NoParams> {
        SnoopRule {
            glob: self.glob,
            sync: self.sync,
            on_write: retag_snoop(self.on_write),
        }
    }
}

impl<Plugins, Params> StorageConfig<Plugins, Params>
where
    Plugins: HList,
    Params: 'static,
{
    pub(crate) fn retag(self) -> StorageConfig<Plugins, NoParams> {
        StorageConfig {
            idmap: self.idmap,
            shared_caches: self.shared_caches,
            lock_broker: self.lock_broker,
            synthetic_files: self.synthetic_files,
            mask_read: self.mask_read,
            snoops: self.snoops.into_iter().map(|rule| rule.retag()).collect(),
            encapsulation: self.encapsulation,
            scan_engine: self.scan_engine,
            carryover_bytes: self.carryover_bytes,
        }
    }
}

impl<Plugins> StorageConfig<Plugins, NoParams>
where
    Plugins: HList,
{
    pub(crate) fn narrow<Head>(self) -> StorageConfig<HCons<Head, Plugins>, NoParams>
    where
        Head: Clone + Send + Sync + 'static,
    {
        StorageConfig {
            idmap: self.idmap,
            shared_caches: self.shared_caches,
            lock_broker: self.lock_broker,
            synthetic_files: self.synthetic_files,
            mask_read: self.mask_read,
            snoops: self
                .snoops
                .into_iter()
                .map(|rule| SnoopRule {
                    glob: rule.glob,
                    sync: rule.sync,
                    on_write: narrow_snoop(rule.on_write),
                })
                .collect(),
            encapsulation: self.encapsulation,
            scan_engine: self.scan_engine,
            carryover_bytes: self.carryover_bytes,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FsReconcileRequest {
    pub cwd: String,
    pub atomic_staging: bool,
    pub fsync: bool,
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
