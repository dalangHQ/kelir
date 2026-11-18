// see: cans/04-terminal-plane.md#Terminal-pipeline
#[path = "log.rs"]
pub mod log;
#[path = "log.flow.rs"]
pub mod log_flow;
#[path = "ops.rs"]
pub mod ops;

pub use log::{LogCursor, LogSegment, LogStream};
pub use ops::{PromptAction, PromptRuleConfig, TerminalOps, TerminalWiring, TokenShieldConfig};
