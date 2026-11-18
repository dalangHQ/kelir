// see: cans/04-terminal-plane.md#Terminal-pipeline
use crate::plugin_model::PluginError;
use crate::terminal::log::LogStream;

// see: cans/04-terminal-plane.md#Terminal-pipeline
pub trait TerminalOps: Send + Sync {
    fn resize(&self, cols: u16, rows: u16) -> Result<(), PluginError>;
    fn feed(&self, bytes: &[u8]) -> Result<(), PluginError>;
    fn export_screen_matrix(&self) -> Result<Vec<u8>, PluginError>;
    fn read_from(&self, port: u32, cursor: u64) -> Result<(Vec<u8>, u64), PluginError>;
    fn append_stream_chunk(&self, stream: LogStream, chunk: &[u8]) -> Result<u64, PluginError>;
    fn record_output(&self, stream: LogStream, bytes: &[u8]) -> Result<(), PluginError>;
    fn write_console(&self, port: u32, bytes: &[u8]) -> Result<(), PluginError> {
        self.record_output(LogStream::Port(port), bytes)
    }
    fn output_filters(&self) -> (bool, bool);
    fn bind_session(&self, session_id: [u8; 16]);
    fn apply_wiring(&self, wiring: &TerminalWiring) -> Result<(), PluginError>;
}

// see: cans/04-terminal-plane.md#Terminal-pipeline
#[derive(Debug, Clone, Default)]
pub struct TerminalWiring {
    pub cols: u16,
    pub rows: u16,
    pub drop_osc52: bool,
    pub strip_title: bool,
}
