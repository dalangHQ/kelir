// see: cans/04-terminal-plane.md#Terminal-pipeline
#![forbid(unsafe_code)]

#[path = "log_segment.run.rs"]
mod log_segment;
#[path = "real_terminal.run.rs"]
mod real_terminal;
#[path = "terminal.flow.rs"]
pub mod terminal;
#[path = "token_shield.run.rs"]
mod token_shield;
#[path = "vt100.run.rs"]
mod vt100;

pub use crate::terminal::terminal_ops;
