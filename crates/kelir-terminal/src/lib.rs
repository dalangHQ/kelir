// see: cans/04-terminal-plane.md#Terminal-pipeline
#![forbid(unsafe_code)]

#[path = "log_segment.flow.rs"]
mod log_segment;
#[path = "real_terminal.run.rs"]
mod real_terminal;
#[path = "terminal.flow.rs"]
pub mod terminal;
#[path = "vt100.flow.rs"]
mod vt100;

pub use crate::terminal::terminal_ops;
