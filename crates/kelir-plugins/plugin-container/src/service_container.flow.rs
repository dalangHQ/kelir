// see: cans/plugins/container.md#Plugin-container
// see: cans/artifacts/code/container-service.rs#ContainerService
use std::ops::Range;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use kelir_interface::plugin_model::PluginError;

use crate::IdMapKind;

// see: cans/artifacts/code/container-service.rs#ContainerService
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

// see: cans/plugins/container.md#Service-exports-(container)
#[derive(Clone)]
pub struct ContainerCtx {
    pub(crate) service: Arc<SharedContainerService>,
}

impl ContainerCtx {
    // see: cans/02-plugin-model.md#KelirPlugin-trait
    pub fn service(&self) -> &Arc<SharedContainerService> {
        &self.service
    }
}

// see: cans/plugins/container.md#UID/GID-mapping-isolation
#[derive(Debug, Default)]
struct ContainerState {
    pinned_subuid: Option<Range<u32>>,
    pinned_subgid: Option<Range<u32>>,
}

// see: cans/plugins/container.md#Service-exports-(container)
#[derive(Debug, Default)]
pub struct SharedContainerService {
    state: Mutex<ContainerState>,
}

fn lock_state(
    state: &Mutex<ContainerState>,
) -> Result<MutexGuard<'_, ContainerState>, PluginError> {
    state
        .lock()
        .map_err(|_| PluginError::Halt("container service state lock poisoned".into()))
}

impl SharedContainerService {
    pub fn new() -> Self {
        Self::default()
    }

    // see: cans/artifacts/code/container-service.rs#configure_runtime
    pub(crate) async fn configure_rootless_runtime(&self) -> Result<(), PluginError> {
        // TODO (L3) [plugin_engine]: configure the rootless Podman and crun runtime environments inside the microVM; slop: Podman/crun env unconfigured, runtime fails or runs unconfined
        Ok(())
    }

    // see: cans/artifacts/code/container-service.rs#mount_scratch
    pub(crate) async fn mount_scratch_device(&self, _scratch: &Path) -> Result<(), PluginError> {
        // TODO (L3) [plugin_engine]: mount the ephemeral scratch device at /var/lib/containers; slop: scratch device unmounted, container writes land on host disk
        Ok(())
    }

    // see: cans/artifacts/code/container-service.rs#ingest_image
    pub(crate) async fn ingest_cached_image(&self, _tarball: &Path) -> Result<String, PluginError> {
        // TODO (L3) [plugin_engine]: delegate ingestion to the external kelir-pack binary; in-process OCI parsing is forbidden; slop: in-process OCI parsing, forbidden path, ingest bypasses kelir-pack
        Err(PluginError::Halt(
            "TODO (L3): container ingest_image delegates to kelir-pack".into(),
        ))
    }

    // see: cans/plugins/container.md#Non-nested-OCI-container-execution
    pub(crate) async fn build_cgroup_delegation_hierarchy(&self) -> Result<(), PluginError> {
        // TODO (L3) [plugin_engine]: construct the synthetic cgroup v2 delegation hierarchy; slop: cgroup hierarchy unbuilt, container escapes CPU/memory limits
        Ok(())
    }

    // see: cans/plugins/container.md#UID/GID-mapping-isolation
    pub(crate) async fn pin_guest_id_map(
        &self,
        subuid: Range<u32>,
        subgid: Range<u32>,
    ) -> Result<(), PluginError> {
        let mut state = lock_state(&self.state)?;
        state.pinned_subuid = Some(subuid);
        state.pinned_subgid = Some(subgid);
        Ok(())
    }

    // see: cans/plugins/container.md#UID/GID-mapping-isolation
    pub(crate) fn pinned_range(&self, kind: IdMapKind) -> Result<Option<Range<u32>>, PluginError> {
        let state = lock_state(&self.state)?;
        Ok(match kind {
            IdMapKind::Uid => state.pinned_subuid.clone(),
            IdMapKind::Gid => state.pinned_subgid.clone(),
        })
    }
}

// see: cans/plugins/container.md#Service-exports-(container)
#[async_trait]
impl ContainerService for SharedContainerService {
    async fn configure_runtime(&mut self) -> Result<(), PluginError> {
        SharedContainerService::configure_rootless_runtime(self).await
    }

    async fn mount_scratch(&mut self, scratch: &Path) -> Result<(), PluginError> {
        SharedContainerService::mount_scratch_device(self, scratch).await
    }

    async fn build_cgroup_delegation(&mut self) -> Result<(), PluginError> {
        SharedContainerService::build_cgroup_delegation_hierarchy(self).await
    }

    async fn ingest_image(&mut self, tarball: &Path) -> Result<String, PluginError> {
        SharedContainerService::ingest_cached_image(self, tarball).await
    }

    async fn pin_id_map(
        &mut self,
        subuid: Range<u32>,
        subgid: Range<u32>,
    ) -> Result<(), PluginError> {
        SharedContainerService::pin_guest_id_map(self, subuid, subgid).await
    }
}
