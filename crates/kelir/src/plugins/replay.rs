// Replay plugin stub.
use crate::plugin::KelirPlugin;

#[derive(Debug, Clone)]
pub struct ReplayConfig {
    pub step_window: u64,
    pub kernel_params: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ReplayPlugin;

impl ReplayPlugin {
    pub fn new(_config: ReplayConfig) -> Self {
        Self
    }
}

impl KelirPlugin for ReplayPlugin {
    type Context = ();
    fn context(&self) -> Self::Context {}
    fn name(&self) -> &'static str {
        "replay"
    }
}
