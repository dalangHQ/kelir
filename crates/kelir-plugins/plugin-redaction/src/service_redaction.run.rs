// see: cans/plugins/redaction.md#Plugin-redaction
use std::path::Path;

use kelir_interface::plugin_model::{FsInterposer, HandlerCtx, PluginError};
use kelir_interface::protocol_wire::{FilterAction, PatchPayload, Verdict};

use crate::service_redaction::RedactionPlugin;

impl FsInterposer for RedactionPlugin {
    fn on_fs_read(&self, path: &Path, buffer: &[u8]) -> Result<Vec<u8>, PluginError> {
        let _ = (path, buffer);
        // TODO (L3) [governance_tools]: mask the read buffer through the redaction service.; slop: the read buffer is never masked, secrets pass through reads unredacted, sensitive data leaks to the caller.
        Ok(buffer.to_vec())
    }

    fn on_fs_unlink(&self, path: &Path, ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        let _ = (path, ctx);
        let _ = PatchPayload::FsPathRedirect {
            target_path: String::new(),
        };
        // TODO (L3) [governance_tools]: redirect the unlink into the soft-delete trash via Verdict::Patch(PatchPayload::FsPathRedirect { target_path }); slop: unlinks delete in place, soft-delete quarantine never fires, deleted data is unrecoverable.
        Err(PluginError::Halt(
            "TODO (L3): soft-delete unlink redirect pending (L3)".into(),
        ))
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
