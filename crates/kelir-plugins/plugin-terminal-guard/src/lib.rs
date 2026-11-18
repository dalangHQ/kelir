// see: cans/plugins/terminal-guard.md#Plugin-terminal-guard
#![forbid(unsafe_code)]

#[path = "notice_box.run.rs"]
pub mod notice_box;
#[path = "service_terminal_guard.flow.rs"]
pub mod service_terminal_guard;

pub use service_terminal_guard::{
    PromptResponderService, PromptRuleConfig, TerminalGuardConfig, TerminalGuardPlugin,
};
