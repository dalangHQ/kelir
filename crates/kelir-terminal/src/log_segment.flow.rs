// see: cans/04-terminal-plane.md#Stream-replay
// see: cans/01-architecture.md#Host-state-root
use std::path::PathBuf;

use kelir_interface::plugin_model::PluginError;
use kelir_interface::terminal::LogStream;

// see: cans/01-architecture.md#Host-state-root
// TODO: [terminal_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow masks the unused item, so the stubbed caller can stay unwired while the build stays green. (deferred to L3)
#[allow(dead_code)]
fn log_file_name(stream: LogStream) -> String {
    match stream {
        LogStream::Port(port) => format!("port-{port}.log"),
        LogStream::Exec { id, lane } => format!("exec-{id}-{lane}.log"),
    }
}

// see: cans/01-architecture.md#Host-state-root
// TODO: [terminal_plane]: write the append-only index.bin chunk index beside the per-stream logs so cursors survive restarts; slop: cursors lost on restart, index never written (deferred to L3)
// TODO: [terminal_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow masks the unused item, so the stubbed caller can stay unwired while the build stays green. (deferred to L3)
#[allow(dead_code)]
pub(crate) fn session_log_path(
    session_id: &[u8; 16],
    stream: LogStream,
) -> Result<PathBuf, PluginError> {
    Ok(kelir_engine::session_lifecycle::session_dir(session_id)?
        .join("logs")
        .join(log_file_name(stream)))
}
