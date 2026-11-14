// see: cans/05-storage-plane.md#Storage-architecture
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use crate::plugin_model::{HandlerCtx, PluginError};

// see: cans/05-storage-plane.md#Write-snoops
pub type SnoopOnWrite = Arc<
    dyn Fn(
            &Path,
            &[u8],
            &HandlerCtx,
        ) -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send>>
        + Send
        + Sync,
>;

// see: cans/02-plugin-model.md#Unified-interposition-pipeline
#[derive(Clone)]
pub struct SnoopRule {
    pub path: String,
    pub sync: bool,
    pub on_write: SnoopOnWrite,
}

impl SnoopRule {
    pub fn new(path: impl Into<String>, sync: bool, on_write: SnoopOnWrite) -> Self {
        Self {
            path: path.into(),
            sync,
            on_write,
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

impl std::fmt::Debug for SnoopRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SnoopRule")
            .field("path", &self.path)
            .field("sync", &self.sync)
            .field("on_write", &"callback")
            .finish()
    }
}
