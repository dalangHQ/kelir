// see: cans/04-terminal-plane.md#Stream-replay
use std::path::PathBuf;

use crate::plugin_model::PluginError;
use crate::terminal::log::{LogCursor, LogSegment, LogStream};

// see: cans/04-terminal-plane.md#Stream-replay
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
