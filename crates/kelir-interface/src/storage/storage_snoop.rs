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
