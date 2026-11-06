// see: cans/plugins/profile.md
#![forbid(unsafe_code)]

// TODO (L3): tests/test_profile.rs per cans/artifacts/config/project-structure.txt.
use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{KelirPlugin, MountScope};

pub trait ConfigurablePlugin: Send + Sync {
    fn configure(&mut self, fragment: &[u8]) -> Result<(), PluginError>;
}

#[derive(Clone)]
pub struct ProfilePlugin;

impl ProfilePlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProfilePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for ProfilePlugin {
    type Context = ();
    fn context(&self) {}
    fn name(&self) -> &'static str {
        "profile"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
}
