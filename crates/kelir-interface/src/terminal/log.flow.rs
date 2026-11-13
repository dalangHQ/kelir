// see: cans/04-terminal-plane.md#Stream-replay
use std::path::PathBuf;

use crate::plugin_model::PluginError;

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
