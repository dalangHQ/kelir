// see: cans/plugins/token-shield.md#Plugin-token-shield
use std::sync::Arc;

use async_trait::async_trait;
use kelir_interface::plugin_model::{
    ConsoleInterposer, KelirPlugin, MountScope, PipelineBinder, PluginError, VirtioDevice,
};

// see: cans/plugins/token-shield.md#Invariants-and-policy-(token-shield)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClampStrategy {
    #[default]
    HeadAndTail,
    HeadOnly,
    TailOnly,
}

// see: cans/plugins/token-shield.md#Invariants-and-policy-(token-shield)
#[derive(Debug, Clone)]
pub struct TokenShieldConfig {
    pub max_bytes_total: usize,
    pub max_lines_total: usize,
    pub clamp_strategy: ClampStrategy,
    pub notice: String,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.token_shield
impl Default for TokenShieldConfig {
    fn default() -> Self {
        Self {
            max_bytes_total: 131072,
            max_lines_total: 500,
            clamp_strategy: ClampStrategy::HeadAndTail,
            notice:
                "\n\u{26a0}\u{fe0f} [SHIELD]: Output clamped to protect model context window.\n"
                    .into(),
        }
    }
}

impl TokenShieldConfig {
    // see: cans/plugins/token-shield.md#Error-and-return-semantics-(token-shield)
    pub fn validate(&self) -> Result<(), PluginError> {
        if self.max_bytes_total == 0 || self.max_lines_total == 0 {
            return Err(PluginError::Halt(
                "EConfig: token-shield: max_bytes_total and max_lines_total must be nonzero".into(),
            ));
        }
        Ok(())
    }
}

// see: cans/plugins/token-shield.md#Service-exports-(token-shield)
#[derive(Debug, Clone)]
pub struct ScrollbackClampService {
    config: TokenShieldConfig,
}

impl ScrollbackClampService {
    // see: cans/plugins/token-shield.md#Service-exports-(token-shield)
    pub fn new(config: TokenShieldConfig) -> Result<Self, PluginError> {
        config.validate()?;
        Ok(Self { config })
    }

    // see: cans/plugins/token-shield.md#Service-exports-(token-shield)
    pub fn config(&self) -> &TokenShieldConfig {
        &self.config
    }

    // see: cans/plugins/token-shield.md#Invariants-and-policy-(token-shield)
    pub fn clamp(&self, grid: &[u8]) -> Vec<u8> {
        crate::clamp::apply_token_shield(
            grid,
            self.config.max_lines_total,
            self.config.max_bytes_total,
        )
    }
}

// see: cans/plugins/token-shield.md#Plugin-token-shield
#[derive(Debug, Clone)]
pub struct TokenShieldPlugin {
    service: Arc<ScrollbackClampService>,
}

impl TokenShieldPlugin {
    // see: cans/plugins/token-shield.md#Plugin-token-shield
    pub fn new(config: TokenShieldConfig) -> Result<Self, PluginError> {
        Ok(Self {
            service: Arc::new(ScrollbackClampService::new(config)?),
        })
    }

    // see: cans/plugins/token-shield.md#Service-exports-(token-shield)
    pub fn service(&self) -> &Arc<ScrollbackClampService> {
        &self.service
    }
}

// see: cans/plugins/token-shield.md#Plugin-token-shield
impl KelirPlugin for TokenShieldPlugin {
    type Context = TokenShieldConfig;

    fn context(&self) -> Self::Context {
        self.service.config().clone()
    }

    fn name(&self) -> &'static str {
        "token-shield"
    }

    fn default_scope(&self) -> MountScope {
        // see: cans/plugins/token-shield.md#Crate-dependencies-(token-shield)
        MountScope::default().with_devices([VirtioDevice::Console])
    }

    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        // see: cans/plugins/token-shield.md#Service-exports-(token-shield)
        let console_stage: Arc<dyn ConsoleInterposer> = Arc::new(self.clone());
        pipeline.bind_console_scoped(console_stage, self.default_scope());
    }
}

// see: cans/plugins/token-shield.md#Plugin-token-shield
#[async_trait]
impl ConsoleInterposer for TokenShieldPlugin {
    async fn on_console_output(&self, _port: u32, chunk: &[u8]) -> Result<Vec<u8>, PluginError> {
        // see: cans/plugins/token-shield.md#Invariants-and-policy-(token-shield)
        Ok(self.service.clamp(chunk))
    }
}
