// Declarative profile facade (L2 stub): `Sandbox::from_config` reads a
// profile file and flattens it into the builder chain the other
// examples build manually. Stubbed: no file is read, no VM boots.
use std::path::PathBuf;

use kelir_interface::errors::PluginError;

use crate::session::SpawnBuilder;

/// A sandbox built from a declarative profile.
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
