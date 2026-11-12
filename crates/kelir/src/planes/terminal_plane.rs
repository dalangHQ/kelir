use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use kelir_interface::plugin_model::PluginError;
use kelir_interface::protocol_wire::TrapEvent;

// see: cans/04-terminal-plane.md#Terminal-pipeline
pub use kelir_core::planes::terminal::TokenShieldConfig;

#[derive(Debug, Clone, Default)]
pub struct PromptStream;

impl PromptStream {
    // TODO (L3) [terminal_plane]: write the matched prompt's response bytes into the guest console stream per cans/04-terminal-plane.md#Host-filter-pipeline-execution-order; Ok(0) drops the answer.; slop: Ok(0) drops answer, console bytes never verified
    pub async fn write(&self, _data: &[u8]) -> Result<usize, PluginError> {
        Ok(0)
    }
}

pub type PromptAction = Arc<
    dyn Fn(
            &TrapEvent,
            PromptStream,
        ) -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send>>
        + Send
        + Sync,
>;

#[derive(Clone)]
pub struct PromptRule {
    pub pattern: String,
    pub timeout: Duration,
    pub verify_foreground: bool,
    pub action: PromptAction,
}

impl std::fmt::Debug for PromptRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PromptRule")
            .field("pattern", &self.pattern)
            .field("timeout", &self.timeout)
            .field("verify_foreground", &self.verify_foreground)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
pub struct PtyConfig {
    pub cols: u16,
    pub rows: u16,
    pub drop_osc52: bool,
    pub strip_title: bool,
    pub token_shield: Option<TokenShieldConfig>,
    pub prompts: Vec<PromptRule>,
    pub stream_engine: String,
}
