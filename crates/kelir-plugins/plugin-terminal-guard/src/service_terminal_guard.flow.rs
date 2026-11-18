// see: cans/plugins/terminal-guard.md#Plugin-terminal-guard
use std::sync::Arc;

use async_trait::async_trait;
use kelir_interface::plugin_model::{
    ConsoleInterposer, KelirPlugin, MountScope, PipelineBinder, PluginError, VirtioDevice,
};

// see: cans/plugins/terminal-guard.md#Invariants-and-policy-(terminal-guard)
#[derive(Clone)]
pub struct PromptRuleConfig {
    pub pattern: String,
    pub timeout_seconds: u64,
    pub verify_foreground: bool,
    pub action: Vec<u8>,
}

impl std::fmt::Debug for PromptRuleConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PromptRuleConfig")
            .field("pattern", &self.pattern)
            .field("timeout_seconds", &self.timeout_seconds)
            .field("verify_foreground", &self.verify_foreground)
            .finish_non_exhaustive()
    }
}

// see: cans/plugins/terminal-guard.md#Invariants-and-policy-(terminal-guard)
#[derive(Debug, Clone, Default)]
pub struct TerminalGuardConfig {
    pub prompts: Vec<PromptRuleConfig>,
    pub notice_template: String,
}

impl TerminalGuardConfig {
    // see: cans/plugins/terminal-guard.md#Error-and-return-semantics-(terminal-guard)
    pub fn validate(&self) -> Result<(), PluginError> {
        if self.prompts.iter().any(|rule| rule.pattern.is_empty()) {
            return Err(PluginError::Halt(
                "EConfig: terminal-guard: prompt rule pattern must be nonempty".into(),
            ));
        }
        Ok(())
    }
}

// see: cans/plugins/terminal-guard.md#Service-exports-(terminal-guard)
#[derive(Debug, Clone)]
pub struct PromptResponderService {
    config: TerminalGuardConfig,
}

impl PromptResponderService {
    // see: cans/plugins/terminal-guard.md#Service-exports-(terminal-guard)
    pub fn new(config: TerminalGuardConfig) -> Result<Self, PluginError> {
        config.validate()?;
        Ok(Self { config })
    }

    // see: cans/plugins/terminal-guard.md#Service-exports-(terminal-guard)
    pub fn config(&self) -> &TerminalGuardConfig {
        &self.config
    }

    // see: cans/plugins/terminal-guard.md#Invariants-and-policy-(terminal-guard)
    pub fn match_rule(&self, _text: &[u8]) -> Option<usize> {
        // L3-GAP: [terminal_guard_prompt_match]: match shadow-stripped text against the plugin-private regex set and gate on verify_foreground pgid ownership; slop: ANSI escapes reach the regex, wrong process answers the prompt
        None
    }

    // see: cans/plugins/terminal-guard.md#Invariants-and-policy-(terminal-guard)
    pub fn render_notice(&self, title: &str, body: &str) -> Vec<u8> {
        crate::notice_box::render_notice_box(title, body)
    }
}

// see: cans/plugins/terminal-guard.md#Plugin-terminal-guard
#[derive(Debug, Clone)]
pub struct TerminalGuardPlugin {
    service: Arc<PromptResponderService>,
}

impl TerminalGuardPlugin {
    // see: cans/plugins/terminal-guard.md#Plugin-terminal-guard
    pub fn new(config: TerminalGuardConfig) -> Result<Self, PluginError> {
        Ok(Self {
            service: Arc::new(PromptResponderService::new(config)?),
        })
    }

    // see: cans/plugins/terminal-guard.md#Service-exports-(terminal-guard)
    pub fn service(&self) -> &Arc<PromptResponderService> {
        &self.service
    }
}

// see: cans/plugins/terminal-guard.md#Plugin-terminal-guard
impl KelirPlugin for TerminalGuardPlugin {
    type Context = TerminalGuardConfig;

    fn context(&self) -> Self::Context {
        self.service.config().clone()
    }

    fn name(&self) -> &'static str {
        "terminal-guard"
    }

    fn default_scope(&self) -> MountScope {
        // see: cans/plugins/terminal-guard.md#Crate-dependencies-(terminal-guard)
        MountScope::default().with_devices([VirtioDevice::Console])
    }

    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        // see: cans/plugins/terminal-guard.md#Service-exports-(terminal-guard)
        let console_stage: Arc<dyn ConsoleInterposer> = Arc::new(self.clone());
        pipeline.bind_console_scoped(console_stage, self.default_scope());
    }
}

// see: cans/plugins/terminal-guard.md#Plugin-terminal-guard
#[async_trait]
impl ConsoleInterposer for TerminalGuardPlugin {
    async fn on_console_output(&self, _port: u32, chunk: &[u8]) -> Result<Vec<u8>, PluginError> {
        // see: cans/plugins/terminal-guard.md#Invariants-and-policy-(terminal-guard)
        if self.service.match_rule(chunk).is_some() {
            // TODO (L3) [terminal_plane]: write the matched rule action bytes to Port 0 and return the Pass verdict; slop: matched prompt rules never fire their action bytes; prompts pass through unguarded.
            return Ok(chunk.to_vec());
        }
        Ok(chunk.to_vec())
    }
}
