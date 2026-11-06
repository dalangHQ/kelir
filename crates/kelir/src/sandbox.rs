use std::path::PathBuf;

use kelir_interface::errors::PluginError;

use crate::session::SpawnBuilder;

#[derive(Debug, Default)]
pub struct Sandbox;

impl Sandbox {
    pub async fn from_config(_path: impl Into<PathBuf>) -> Result<Self, PluginError> {
        Ok(Self)
    }

    pub async fn boot(self) -> Result<Self, PluginError> {
        Ok(self)
    }

    pub fn exec(&mut self, _binary: &str) -> SpawnBuilder {
        SpawnBuilder::default()
    }

    pub async fn export_spec(&self, _path: impl Into<PathBuf>) -> Result<(), PluginError> {
        Ok(())
    }

    pub async fn terminate(&self) -> Result<(), PluginError> {
        Ok(())
    }
}
