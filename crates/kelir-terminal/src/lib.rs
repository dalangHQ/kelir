// see: cans/04-terminal-plane.md#Terminal-pipeline
#![forbid(unsafe_code)]

#[path = "log_segment.run.rs"]
pub mod log_segment;
#[path = "real_terminal.run.rs"]
pub mod real_terminal;
#[path = "token_shield.run.rs"]
pub mod token_shield;
#[path = "vt100.run.rs"]
pub mod vt100;

pub use real_terminal::RealTerminalOps;
