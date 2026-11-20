// see: cans/plugins/container.md#Plugin-container
// see: cans/artifacts/code/container-service.rs#ContainerService
use std::ops::Range;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use kelir_interface::errno_types::Errno;
use kelir_interface::plugin_model::{
    HandlerCtx, KelirPlugin, LifecycleCtx, LifecycleParticipant, MountScope, PipelineBinder,
    PluginError, TrapInterposer,
};
use kelir_interface::protocol_wire::{TrapEvent, Verdict};

use crate::{
    ContainerConfig, ContainerPlugin, IdMapAllocation, IdMapKind, NEWGIDMAP, NEWUIDMAP, PODMAN,
    SUBGID_PATH, SUBUID_PATH,
};

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

// see: cans/plugins/container.md#Service-exports-(container)
pub type ContainerHandle = ContainerCtx;

impl ContainerCtx {
    // see: cans/02-plugin-model.md#KelirPlugin-trait
    pub fn service(&self) -> &Arc<SharedContainerService> {
        &self.service
    }

    // see: cans/plugins/container.md#Plugin-container
    // see: cans/plugins/container.md#Service-exports-(container)
    pub async fn ingest_image(&self, tarball: &Path) -> Result<String, PluginError> {
        crate::service_container_run::check_tarball_readable(tarball)?;
        self.service.ingest_cached_image(tarball).await
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
        // L3-GAP: [plugin_engine]: configure the rootless Podman and crun runtime environments inside the microVM; slop: Podman/crun env unconfigured, runtime fails or runs unconfined
        Ok(())
    }

    // see: cans/artifacts/code/container-service.rs#mount_scratch
    pub(crate) async fn mount_scratch_device(&self, _scratch: &Path) -> Result<(), PluginError> {
        // L3-GAP: [plugin_engine]: mount the ephemeral scratch device at /var/lib/containers; slop: scratch device unmounted, container writes land on host disk
        Ok(())
    }

    // see: cans/artifacts/code/container-service.rs#ingest_image
    pub(crate) async fn ingest_cached_image(&self, _tarball: &Path) -> Result<String, PluginError> {
        // L3-GAP: [plugin_engine]: delegate ingestion to the external kelir-pack binary; in-process OCI parsing is forbidden; slop: in-process OCI parsing, forbidden path, ingest bypasses kelir-pack
        Err(PluginError::Halt(
            "TODO (L3): container ingest_image delegates to kelir-pack".into(),
        ))
    }

    // see: cans/plugins/container.md#Non-nested-OCI-container-execution
    pub(crate) async fn build_cgroup_delegation_hierarchy(&self) -> Result<(), PluginError> {
        // L3-GAP: [plugin_engine]: construct the synthetic cgroup v2 delegation hierarchy; slop: cgroup hierarchy unbuilt, container escapes CPU/memory limits
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

// see: cans/plugins/container.md#UID/GID-mapping-isolation
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SubidOwner {
    name: Option<String>,
    uid: u32,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.container
// see: cans/plugins/profile.md#Default-zero-capability-invariant
impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            runtime: "crun".into(),
            isolate_var_lib_containers: true,
            allow_unsandboxed: false,
            pin_subuid_ranges: true,
            scratch_device: None,
        }
    }
}

impl ContainerPlugin {
    pub fn new(config: ContainerConfig) -> Self {
        Self {
            config,
            service: Arc::new(SharedContainerService::new()),
        }
    }

    // see: cans/plugins/container.md#Service-exports-(container)
    pub fn ctx(&self) -> ContainerCtx {
        ContainerCtx {
            service: Arc::clone(&self.service),
        }
    }

    // see: cans/plugins/container.md#UID/GID-mapping-isolation
    pub(crate) async fn eval_id_map_trap(
        &self,
        kind: IdMapKind,
        event: &TrapEvent,
    ) -> Result<Verdict, PluginError> {
        // L3-GAP: [plugin_engine]: subuid/subgid range resolution against the guest user policy beyond the pinned containment check; slop: subuid/subgid beyond pin unchecked, guest uid escapes containment
        let Ok(allocations) = parse_id_map_allocations(&event.argv) else {
            return Ok(reject_eperm(
                "malformed newuidmap/newgidmap allocation request",
            ));
        };
        let Some(pinned) = self.service.pinned_range(kind)? else {
            return Ok(reject_eperm(
                "no subuid/subgid range pinned to unprivileged guest users",
            ));
        };
        if allocations
            .iter()
            .any(|alloc| !allocation_within(*alloc, &pinned))
        {
            return Ok(reject_eperm(
                "id map allocation outside the pinned unprivileged guest range",
            ));
        }
        Ok(Verdict::Pass)
    }

    // see: cans/plugins/container.md#Error-and-return-semantics-(container)
    pub(crate) fn eval_mount_policy(&self, event: &TrapEvent) -> Verdict {
        let binary = binary_name(&event.binary_path);
        if binary != PODMAN && binary != self.config.runtime.as_str() {
            return Verdict::Pass;
        }
        if event.argv.iter().any(|arg| arg == "--privileged") {
            return reject_eperm("privileged container escape attempt");
        }
        for (index, arg) in event.argv.iter().enumerate() {
            if arg == "--device"
                && event
                    .argv
                    .get(index + 1)
                    .is_some_and(|device| device.starts_with("/dev/"))
            {
                return reject_eperm("raw device mount request");
            }
            if arg
                .strip_prefix("--device=")
                .is_some_and(|device| device.starts_with("/dev/"))
            {
                return reject_eperm("raw device mount request");
            }
        }
        // L3-GAP: [plugin_engine]: mount policy table for the OCI runtime config path (capability sets, device allowlist); slop: mount policy table missing, capabilities/devices unfiltered at runtime
        Verdict::Pass
    }
}

impl Default for ContainerPlugin {
    fn default() -> Self {
        Self::new(ContainerConfig::default())
    }
}

// see: cans/plugins/container.md#Non-nested-OCI-container-execution
#[async_trait]
impl LifecycleParticipant for ContainerPlugin {
    async fn on_vm_ready(&self, _ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        // see: cans/plugins/container.md#Non-nested-OCI-container-execution
        if !self.config.isolate_var_lib_containers && !self.config.allow_unsandboxed {
            return Err(PluginError::Halt("ESecurityViolation: isolate_var_lib_containers is required; set allow_unsandboxed to opt out explicitly".to_string()));
        }
        self.service.configure_rootless_runtime().await?;
        if self.config.isolate_var_lib_containers {
            let scratch = self.config.scratch_device.as_deref().ok_or_else(|| {
                PluginError::Halt(
                    "scratch device path missing for /var/lib/containers isolation".into(),
                )
            })?;
            self.service.mount_scratch_device(scratch).await?;
        }
        self.service.build_cgroup_delegation_hierarchy().await?;
        if self.config.pin_subuid_ranges {
            let owner = invoking_subid_owner()?;
            let (subuid, subgid) = resolve_unprivileged_ranges(&owner)?;
            self.service.pin_guest_id_map(subuid, subgid).await?;
        }
        // L3-GAP: [plugin_engine]: apply the pinned ranges through the host idmap configuration in StorageConfig; slop: pinned ranges never applied to idmap, host ids leak into guest
        Ok(())
    }
}

// see: cans/plugins/container.md#UID/GID-mapping-isolation
#[async_trait]
impl TrapInterposer for ContainerPlugin {
    async fn on_trap(&self, event: &TrapEvent, _ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        match id_map_kind(&event.binary_path) {
            Some(kind) => self.eval_id_map_trap(kind, event).await,
            None => Ok(self.eval_mount_policy(event)),
        }
    }
}

// see: cans/plugins/container.md#Plugin-container
impl KelirPlugin for ContainerPlugin {
    type Context = ContainerCtx;
    fn context(&self) -> Self::Context {
        self.ctx()
    }
    fn name(&self) -> &'static str {
        "container"
    }

    // see: cans/plugins/container.md#UID/GID-mapping-isolation
    fn default_scope(&self) -> MountScope {
        // L3-GAP: [pe_20_same_path_different_identity_never_matches]: gate-2 canonical (st_dev, st_ino, binary_blake3) identity per cans/11-protocol.md#Trap-fast-path; slop: path string matched, swapped (dev,ino,blake3) passes as trusted git
        MountScope::default().with_binaries([
            NEWUIDMAP,
            NEWGIDMAP,
            PODMAN,
            self.config.runtime.as_str(),
        ])
    }

    // see: cans/plugins/container.md#Non-nested-OCI-container-execution
    fn fork_state(
        &self,
        _child_session_id: &str,
    ) -> Box<dyn kelir_interface::plugin_model::AnyPlugin> {
        // see: cans/08-lifecycle.md#CoW-fork
        Box::new(Self::new(self.config.clone()))
    }

    // see: cans/plugins/container.md#Service-exports-(container)
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        pipeline.bind_trap(Arc::new(self.clone()));
        // see: cans/plugins/container.md#Non-nested-OCI-container-execution
        let lifecycle_stage: Arc<dyn LifecycleParticipant> = Arc::new(self.clone());
        pipeline.bind_lifecycle_scoped(lifecycle_stage, self.default_scope());
    }
}

// see: cans/plugins/container.md#UID/GID-mapping-isolation
pub(crate) fn id_map_kind(binary_path: &str) -> Option<IdMapKind> {
    match binary_name(binary_path) {
        NEWUIDMAP => Some(IdMapKind::Uid),
        NEWGIDMAP => Some(IdMapKind::Gid),
        _ => None,
    }
}

fn binary_name(binary_path: &str) -> &str {
    Path::new(binary_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
}

// see: cans/plugins/container.md#UID/GID-mapping-isolation
fn parse_id_map_allocations(argv: &[String]) -> Result<Vec<IdMapAllocation>, String> {
    let pid = argv
        .get(1)
        .ok_or_else(|| "id map trap missing pid argument".to_string())?;
    pid.parse::<u32>()
        .map_err(|_| "id map trap pid argument is not numeric".to_string())?;
    let rest = argv
        .get(2..)
        .ok_or_else(|| "id map trap missing allocation triples".to_string())?;
    if rest.is_empty() || rest.len() % 3 != 0 {
        return Err("id map trap allocation fields are not lower/upper/count triples".to_string());
    }
    let mut allocations = Vec::with_capacity(rest.len() / 3);
    for triple in rest.chunks(3) {
        let lower = triple[0]
            .parse::<u32>()
            .map_err(|_| "id map lower id is not numeric".to_string())?;
        let upper = triple[1]
            .parse::<u32>()
            .map_err(|_| "id map upper id is not numeric".to_string())?;
        let count = triple[2]
            .parse::<u32>()
            .map_err(|_| "id map count is not numeric".to_string())?;
        if count == 0 {
            return Err("id map allocation count is zero".to_string());
        }
        upper
            .checked_add(count)
            .ok_or_else(|| "id map allocation overflows the id range".to_string())?;
        allocations.push(IdMapAllocation {
            lower,
            upper,
            count,
        });
    }
    Ok(allocations)
}

// see: cans/plugins/container.md#UID/GID-mapping-isolation
fn allocation_within(alloc: IdMapAllocation, pinned: &Range<u32>) -> bool {
    if alloc.upper < pinned.start {
        return false;
    }
    match alloc.upper.checked_add(alloc.count) {
        Some(end) => end <= pinned.end,
        None => false,
    }
}

// see: cans/plugins/container.md#Error-and-return-semantics-(container)
fn reject_eperm(reason: &str) -> Verdict {
    Verdict::Drop {
        errno: Errno::Eperm,
        reason: reason.to_string(),
    }
}

// see: cans/plugins/container.md#UID/GID-mapping-isolation
pub(crate) fn invoking_subid_owner() -> Result<SubidOwner, PluginError> {
    let uid = crate::service_container_run::effective_host_uid()?;
    Ok(SubidOwner {
        name: crate::service_container_run::passwd_name_for_uid(uid),
        uid,
    })
}

// see: cans/plugins/container.md#UID/GID-mapping-isolation
fn parse_subid_file(path: &str, owner: &SubidOwner) -> Result<Option<Range<u32>>, PluginError> {
    let contents = crate::service_container_run::read_subid_file(path)?;
    let mut matched = None;
    for line in contents.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let mut fields = line.split(':');
        let (user, lower, count) = match (fields.next(), fields.next(), fields.next()) {
            (Some(user), Some(lower), Some(count)) if fields.next().is_none() => {
                (user, lower, count)
            }
            _ => {
                return Err(PluginError::Halt(format!(
                    "EINVAL: {path} line is not owner:lower:count: {line}"
                )));
            }
        };
        let lower = lower.parse::<u32>().map_err(|_| {
            PluginError::Halt(format!("EINVAL: {path} lower id is not numeric: {line}"))
        })?;
        let count = count.parse::<u32>().map_err(|_| {
            PluginError::Halt(format!("EINVAL: {path} count is not numeric: {line}"))
        })?;
        if count == 0 {
            return Err(PluginError::Halt(format!(
                "EINVAL: {path} count is zero: {line}"
            )));
        }
        let end = lower.checked_add(count).ok_or_else(|| {
            PluginError::Halt(format!("EINVAL: {path} range overflows u32: {line}"))
        })?;
        let owner_match = owner.name.as_deref().is_some_and(|name| user == name)
            || user.parse::<u32>() == Ok(owner.uid);
        if owner_match {
            if matched.is_some() {
                return Err(PluginError::Halt(format!(
                    "EINVAL: {path} carries duplicate entries for uid {}",
                    owner.uid
                )));
            }
            matched = Some(lower..end);
        }
    }
    Ok(matched)
}

// see: cans/plugins/container.md#UID/GID-mapping-isolation
pub(crate) fn resolve_unprivileged_ranges(
    owner: &SubidOwner,
) -> Result<(Range<u32>, Range<u32>), PluginError> {
    let subuid = parse_subid_file(SUBUID_PATH, owner)?.ok_or_else(|| {
        PluginError::Halt(format!(
            "ENOENT: no subuid range allocated to uid {}",
            owner.uid
        ))
    })?;
    let subgid = parse_subid_file(SUBGID_PATH, owner)?.ok_or_else(|| {
        PluginError::Halt(format!(
            "ENOENT: no subgid range allocated to uid {}",
            owner.uid
        ))
    })?;
    Ok((subuid, subgid))
}
