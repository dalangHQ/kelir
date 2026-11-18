// see: cans/04-terminal-plane.md#Stream-replay
use std::path::PathBuf;

// see: cans/04-terminal-plane.md#Stream-replay
pub struct LogCursor(pub u64);

// see: cans/04-terminal-plane.md#Stream-replay
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogStream {
    Port(u32),
    Exec { id: u64, lane: u8 },
}

// see: cans/04-terminal-plane.md#Stream-replay
pub struct LogSegment {
    pub stream: LogStream,
    pub head: LogCursor,
    pub bytes: Vec<u8>,
    pub log_path: PathBuf,
}
