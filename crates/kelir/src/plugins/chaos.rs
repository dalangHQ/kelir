// Chaos plugin stub.
use crate::plugin::KelirPlugin;

#[derive(Debug, Clone)]
pub struct NetworkChaos {
    pub target: String,
    pub base_ms: u64,
    pub jitter_ms: u64,
    pub drop_rate: f64,
    pub inject_rst: bool,
    pub bandwidth_kbps: Option<u64>,
    pub direction: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FsChaos {
    pub target_path: String,
    pub simulate_error: String,
    pub error_probability: f64,
}

#[derive(Debug, Clone)]
pub struct ChaosConfig {
    pub network: Vec<NetworkChaos>,
    pub fs: Vec<FsChaos>,
}

#[derive(Debug, Clone, Default)]
pub struct ChaosPlugin;

impl ChaosPlugin {
    pub fn new(_config: ChaosConfig) -> Self {
        Self
    }
}

impl KelirPlugin for ChaosPlugin {
    type Context = ();
    fn context(&self) -> Self::Context {}
    fn name(&self) -> &'static str {
        "chaos"
    }
}
