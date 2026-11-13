// see: cans/04-terminal-plane.md#Terminal-pipeline
#[path = "log.flow.rs"]
pub mod log;
#[path = "ops.flow.rs"]
pub mod ops;

pub use log::{LogCursor, LogSegment, LogStream};
pub use ops::{PromptAction, PromptRuleConfig, TerminalOps, TerminalWiring, TokenShieldConfig};
