// see: cans/plugins/container.md
use async_trait::async_trait;
use kelir_interface::errors::PluginError;
use std::ops::Range;
use std::path::Path;

#[async_trait]
pub trait ContainerService: Send + Sync {
    async fn configure_runtime(&mut self) -> Result<(), PluginError>;
    async fn mount_scratch(&mut self, scratch: &Path) -> Result<(), PluginError>;
    async fn build_cgroup_delegation(&mut self) -> Result<(), PluginError>;
    async fn ingest_image(&mut self, tarball: &Path) -> Result<String, PluginError>;
    async fn pin_id_map(
        &mut self,
        subuid: Range<u32>,
        subgid: Range<u32>,
    ) -> Result<(), PluginError>;
}

#[async_trait]
impl ContainerService for crate::ContainerPlugin {
    async fn configure_runtime(&mut self) -> Result<(), PluginError> {
        // see: cans/plugins/container.md
        Ok(())
    }
    async fn mount_scratch(&mut self, _scratch: &Path) -> Result<(), PluginError> {
        // see: cans/plugins/container.md
        Ok(())
    }
    async fn build_cgroup_delegation(&mut self) -> Result<(), PluginError> {
        // see: cans/plugins/container.md
        Ok(())
    }
    async fn ingest_image(&mut self, _tarball: &Path) -> Result<String, PluginError> {
        // see: cans/plugins/container.md
        Ok(String::new())
    }
    async fn pin_id_map(
        &mut self,
        _subuid: Range<u32>,
        _subgid: Range<u32>,
    ) -> Result<(), PluginError> {
        // see: cans/plugins/container.md
        Ok(())
    }
}
