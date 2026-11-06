// Supply-chain plugin stub.
use crate::plugin::KelirPlugin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Npm,
    Pip,
    Cargo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecyclePolicy {
    DenyInDependencyDirs,
}

#[derive(Debug, Clone)]
pub struct SupplyChainConfig {
    pub managers: Vec<PackageManager>,
    pub policy: LifecyclePolicy,
}

#[derive(Debug, Clone, Default)]
pub struct SupplyChainPlugin;

impl SupplyChainPlugin {
    pub fn new(_config: SupplyChainConfig) -> Self {
        Self
    }
}

impl KelirPlugin for SupplyChainPlugin {
    type Context = ();
    fn context(&self) -> Self::Context {}
    fn name(&self) -> &'static str {
        "supply-chain"
    }
}
