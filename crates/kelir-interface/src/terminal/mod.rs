// see: cans/04-terminal-plane.md#Terminal-pipeline
// see: cans/04-terminal-plane.md#Stream-replay
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use crate::plugin_model::PluginError;
use crate::protocol_wire::TrapEvent;

// see: cans/04-terminal-plane.md#Terminal-pipeline
pub trait TerminalOps: Send + Sync {
    fn resize(&self, cols: u16, rows: u16) -> Result<(), PluginError>;
    fn feed(&self, bytes: &[u8]) -> Result<(), PluginError>;
    fn export_screen_matrix(&self) -> Result<Vec<u8>, PluginError>;
    fn read_from(&self, port: u32, cursor: u64) -> Result<(Vec<u8>, u64), PluginError>;
    fn append_stream_chunk(&self, stream: LogStream, chunk: &[u8]) -> Result<u64, PluginError>;
    fn record_output(&self, stream: LogStream, bytes: &[u8]) -> Result<(), PluginError>;
    fn output_filters(&self) -> (bool, bool);
    fn token_shield(&self) -> Option<TokenShieldConfig>;
    fn prompts(&self) -> Vec<PromptRuleConfig>;
    fn bind_session(&self, session_id: [u8; 16]);
    fn apply_wiring(&self, wiring: &TerminalWiring) -> Result<(), PluginError>;
}

// see: cans/04-terminal-plane.md#Terminal-pipeline
#[derive(Debug, Clone)]
pub struct TokenShieldConfig {
    pub max_bytes_total: usize,
    pub max_lines_total: usize,
    pub clamp_strategy: String,
    pub notice: String,
}

// see: cans/04-terminal-plane.md#Terminal-pipeline
pub type PromptAction = Arc<
    dyn Fn(&TrapEvent) -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send>>
        + Send
        + Sync,
>;

// see: cans/04-terminal-plane.md#Terminal-pipeline
#[derive(Clone)]
pub struct PromptRuleConfig {
    pub pattern: String,
    pub timeout: Duration,
    pub verify_foreground: bool,
    pub action: PromptAction,
}

impl std::fmt::Debug for PromptRuleConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PromptRuleConfig")
            .field("pattern", &self.pattern)
            .field("timeout", &self.timeout)
            .field("verify_foreground", &self.verify_foreground)
            .finish_non_exhaustive()
    }
}

// see: cans/04-terminal-plane.md#Terminal-pipeline
#[derive(Debug, Clone, Default)]
pub struct TerminalWiring {
    pub cols: u16,
    pub rows: u16,
    pub drop_osc52: bool,
    pub strip_title: bool,
    pub token_shield: Option<TokenShieldConfig>,
    pub prompts: Vec<PromptRuleConfig>,
}

pub struct LogCursor(pub u64);

impl LogCursor {
    pub fn start() -> Self {
        Self(0)
    }

    pub fn advance(&self, len: u64) -> Result<Self, PluginError> {
        self.0
            .checked_add(len)
            .map(Self)
            .ok_or_else(|| PluginError::Halt("log cursor overflow".into()))
    }
}

// see: cans/04-terminal-plane.md#Stream-replay
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogStream {
    Port(u32),
    Exec { id: u64, lane: u8 },
}

pub struct LogSegment {
    pub stream: LogStream,
    pub head: LogCursor,
    pub bytes: Vec<u8>,
    pub log_path: PathBuf,
}

impl LogSegment {
    pub fn new(stream: LogStream, log_path: PathBuf) -> Self {
        Self {
            stream,
            head: LogCursor::start(),
            bytes: Vec::new(),
            log_path,
        }
    }
}
