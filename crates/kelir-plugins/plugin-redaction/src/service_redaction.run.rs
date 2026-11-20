// see: cans/plugins/redaction.md#Plugin-redaction
use std::path::Path;

use kelir_interface::plugin_model::{FsInterposer, HandlerCtx, PluginError};
use kelir_interface::protocol_wire::FilterAction;

use crate::service_redaction::RedactionPlugin;

impl FsInterposer for RedactionPlugin {
    fn on_fs_read(&self, path: &Path, buffer: &[u8]) -> Result<Vec<u8>, PluginError> {
        let _ = (path, buffer);
        // TODO (L3) [governance_tools]: mask the read buffer through the redaction service.; slop: the read buffer is never masked, secrets pass through reads unredacted, sensitive data leaks to the caller.
        Ok(buffer.to_vec())
    }

    fn on_fs_write(
        &self,
        path: &Path,
        data: &mut [u8],
        pid: u32,
        ctx: &HandlerCtx,
    ) -> Result<FilterAction, PluginError> {
        let _ = (path, data, pid, ctx);
        // TODO (L3) [governance_tools]: run the write snoop check through the redaction service.; slop: the write snoop check never runs, secrets pass through writes unredacted, sensitive data leaks to the sink.
        Ok(FilterAction::Forward)
    }
}
