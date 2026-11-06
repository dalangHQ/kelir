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
        // TODO: Configure rootless Podman/crun runtime environments
        Ok(())
    }
    async fn mount_scratch(&mut self, _scratch: &Path) -> Result<(), PluginError> {
        // TODO: Mount ephemeral scratch device at /var/lib/containers
        Ok(())
    }
    async fn build_cgroup_delegation(&mut self) -> Result<(), PluginError> {
        // TODO: Construct synthetic cgroup v2 delegation hierarchy
        Ok(())
    }
    async fn ingest_image(&mut self, _tarball: &Path) -> Result<String, PluginError> {
        // TODO: Ingest pre-cached OCI image tarball, return image reference
        Ok(String::new())
    }
    async fn pin_id_map(
        &mut self,
        _subuid: Range<u32>,
        _subgid: Range<u32>,
    ) -> Result<(), PluginError> {
        // TODO: Pin subuid/subgid ranges to unprivileged guest ranges
        Ok(())
    }
}
