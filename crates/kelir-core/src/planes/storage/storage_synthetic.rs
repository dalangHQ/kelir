// see: cans/05-storage-plane.md#Synthetic-FUSE-inodes
use std::path::Path;
use std::sync::Arc;

use kelir_interface::plugin_model::{FsInodeAttr, PluginError};

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
        mode: u32,
    },
    Dynamic {
        path: String,
        generator: DynamicFileGenerator,
    },
}

impl SyntheticFile {
    pub fn static_(path: &str, content: &str) -> Self {
        // see: cans/05-storage-plane.md#Synthetic-FUSE-inodes
        Self::static_with_mode(path, content, 0o400)
    }

    // see: cans/05-storage-plane.md#Synthetic-FUSE-inodes
    pub fn static_with_mode(path: &str, content: &str, mode: u32) -> Self {
        Self::Static {
            path: path.to_string(),
            content: content.to_string(),
            mode,
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

    pub fn path(&self) -> &str {
        match self {
            Self::Static { path, .. } | Self::Dynamic { path, .. } => path,
        }
    }

    // see: cans/05-storage-plane.md#Synthetic-FUSE-inodes
    pub fn lookup_attr(&self, path: &Path) -> Result<Option<FsInodeAttr>, PluginError> {
        if Path::new(self.path()) != path {
            return Ok(None);
        }
        match self {
            Self::Static { content, mode, .. } => Ok(Some(FsInodeAttr {
                mode: *mode,
                uid: 0,
                gid: 0,
                size: content.len() as u64,
                content: content.clone().into_bytes(),
            })),
            Self::Dynamic { generator, .. } => {
                let file = generator(path)?;
                Ok(Some(FsInodeAttr {
                    // see: cans/02-plugin-model.md#FsInodeAttr-carries-full-mode,-uid,-gid,-size,-content
                    mode: 0o400,
                    uid: 0,
                    gid: 0,
                    size: file.content.len() as u64,
                    content: file.content.into_bytes(),
                }))
            }
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
