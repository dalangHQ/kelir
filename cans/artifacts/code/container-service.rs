// crates/kelir-plugins/plugin-container/src/service_container.flow.rs
//
// The ContainerService surface: what the container plugin exposes
// through PipelineBinder for container lifecycle tracking. Each method
// is one behavior from the plugin spec; the plugin invokes them from
// its LifecycleParticipant and TrapInterposer implementations, never
// from the guest. No nested hypervisor is involved at any point:
// images come from pre-cached host tarballs and the runtime is
// rootless Podman/crun inside the microVM.
//
// Owned by: cans/plugins/container.md#Service-exports-(container)

use async_trait::async_trait;
use std::ops::Range;
use std::path::Path;
use kelir_interface::plugin_model::PluginError;

/// Container lifecycle service. Implemented by the container plugin;
/// the guest workload runs containers with plain `podman run` inside
/// the microVM once this service has prepared the environment.
/// Privileged escape attempts and raw device mounts never reach the
/// service: the trap filter rejects them with EPERM
/// (Verdict::Drop { errno: Some("EPERM") }).
#[async_trait]
pub trait ContainerService: Send + Sync {
    /// Configures the rootless Podman and crun runtime environments
    /// inside the microVM.
    async fn configure_runtime(&mut self) -> Result<(), PluginError>;

    /// Mounts the ephemeral scratch device at /var/lib/containers,
    /// isolating storage diffs from the guest root filesystem.
    /// `scratch` is the host scratch device path.
    async fn mount_scratch(&mut self, scratch: &Path) -> Result<(), PluginError>;

    /// Constructs the synthetic cgroup v2 delegation hierarchy.
    /// Invoked from on_vm_ready, before any container starts.
    async fn build_cgroup_delegation(&mut self) -> Result<(), PluginError>;

    /// Ingests one pre-cached OCI image tarball from the host package
    /// cache. Returns the image reference the workload passes to
    /// `podman run`. No image translation runs; the tarball is used
    /// as cached.
    async fn ingest_image(&mut self, tarball: &Path) -> Result<String, PluginError>;

    /// Pins the subuid and subgid ranges to unprivileged guest user
    /// ranges. newuidmap/newgidmap allocations outside the pinned
    /// ranges are intercepted by the trap filter and rejected.
    async fn pin_id_map(
        &mut self,
        subuid: Range<u32>,
        subgid: Range<u32>,
    ) -> Result<(), PluginError>;
}
