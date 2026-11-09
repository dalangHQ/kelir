use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use kelir_interface::plugin_model::PluginError;
use kelir_interface::protocol_wire::TrapEvent;

#[derive(Debug, Clone, Default)]
pub struct PromptStream;

impl PromptStream {
    // TODO (L3): write the matched prompt's response bytes into the guest console stream per cans/04-terminal-plane.md#Host-filter-pipeline-execution-order; Ok(0) drops the answer.
    pub async fn write(&self, _data: &[u8]) -> Result<usize, PluginError> {
        Ok(0)
    }
}

#[derive(Debug, Clone)]
pub struct TokenShield {
    pub max_bytes_total: usize,
    pub max_lines_total: usize,
    pub clamp_strategy: String,
    pub notice: String,
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
    pub token_shield: Option<TokenShield>,
    pub prompts: Vec<PromptRule>,
    pub stream_engine: String,
}
