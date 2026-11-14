// see: cans/plugins/container.md#Plugin-container
#![forbid(unsafe_code)]

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{
    HandlerCtx, KelirPlugin, LifecycleCtx, LifecycleParticipant, MountScope, PipelineBinder,
    PluginDeps, TrapInterposer,
};
use kelir_interface::protocol_wire::{Errno, TrapEvent, Verdict};

#[path = "service_container.flow.rs"]
pub mod service_container;

pub use service_container::{ContainerCtx, ContainerService, SharedContainerService};

// see: cans/plugins/container.md#UID/GID-mapping-isolation
const NEWUIDMAP: &str = "newuidmap";

// see: cans/plugins/container.md#UID/GID-mapping-isolation
const NEWGIDMAP: &str = "newgidmap";

// see: cans/plugins/container.md#Non-nested-OCI-container-execution
const PODMAN: &str = "podman";

// see: cans/05-storage-plane.md#Host-inode-lock-broker
const SUBUID_PATH: &str = "/etc/subuid";

// see: cans/05-storage-plane.md#Host-inode-lock-broker
const SUBGID_PATH: &str = "/etc/subgid";

// see: cans/plugins/container.md#UID/GID-mapping-isolation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdMapKind {
    Uid,
    Gid,
}

// see: cans/plugins/container.md#UID/GID-mapping-isolation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdMapAllocation {
    pub lower: u32,
    pub upper: u32,
    pub count: u32,
}

// see: cans/plugins/container.md#UID/GID-mapping-isolation
#[derive(Debug, Clone, PartialEq, Eq)]
struct SubidOwner {
    name: Option<String>,
    uid: u32,
}

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.container
// see: cans/plugins/profile.md#Default-zero-capability-invariant
#[derive(Debug, Clone)]
pub struct ContainerConfig {
    pub runtime: String,
    pub isolate_var_lib_containers: bool,
    pub allow_unsandboxed: bool,
    pub pin_subuid_ranges: bool,
    pub scratch_device: Option<PathBuf>,
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

// see: cans/plugins/container.md#Plugin-container
#[derive(Clone, PluginDeps)]
pub struct ContainerPlugin {
    config: ContainerConfig,
    service: Arc<SharedContainerService>,
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
    async fn eval_id_map_trap(
        &self,
        kind: IdMapKind,
        event: &TrapEvent,
    ) -> Result<Verdict, PluginError> {
        // TODO (L3) [plugin_engine]: subuid/subgid range resolution against the guest user policy beyond the pinned containment check; slop: subuid/subgid beyond pin unchecked, guest uid escapes containment
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
    fn eval_mount_policy(&self, event: &TrapEvent) -> Verdict {
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
        // TODO (L3) [plugin_engine]: mount policy table for the OCI runtime config path (capability sets, device allowlist); slop: mount policy table missing, capabilities/devices unfiltered at runtime
        Verdict::Pass
    }
}

impl Default for ContainerPlugin {
    fn default() -> Self {
        Self::new(ContainerConfig::default())
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
        // TODO (L3) [pe_20_same_path_different_identity_never_matches]: gate-2 canonical (st_dev, st_ino, binary_blake3) identity per cans/11-protocol.md#Trap-fast-path; slop: path string matched, swapped (dev,ino,blake3) passes as trusted git
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

// see: cans/02-plugin-model.md#KelirPlugin-trait

// see: cans/plugins/container.md#Non-nested-OCI-container-execution
#[async_trait]
impl LifecycleParticipant for ContainerPlugin {
    async fn on_vm_ready(&self, _ctx: &mut LifecycleCtx<'_>) -> Result<(), PluginError> {
        // see: cans/plugins/container.md#Non-nested-OCI-container-execution
        if !self.config.isolate_var_lib_containers && !self.config.allow_unsandboxed {
            return Err(PluginError::reject(
                Errno::Esecurityviolation,
                "isolate_var_lib_containers is required; set allow_unsandboxed to opt out explicitly",
            ));
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
        // TODO (L3) [plugin_engine]: apply the pinned ranges through the host idmap configuration in StorageConfig; slop: pinned ranges never applied to idmap, host ids leak into guest
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

// see: cans/plugins/container.md#UID/GID-mapping-isolation
fn id_map_kind(binary_path: &str) -> Option<IdMapKind> {
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
        errno: Some(Errno::Eperm),
        reason: reason.to_string(),
    }
}

// see: cans/plugins/container.md#UID/GID-mapping-isolation
fn invoking_subid_owner() -> Result<SubidOwner, PluginError> {
    let uid = effective_host_uid()?;
    Ok(SubidOwner {
        name: passwd_name_for_uid(uid),
        uid,
    })
}

// see: cans/plugins/container.md#UID/GID-mapping-isolation
fn effective_host_uid() -> Result<u32, PluginError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let meta = std::fs::metadata("/proc/self").map_err(|e| {
            PluginError::reject(Errno::Enoent, format!("host identity probe failed: {e}"))
        })?;
        Ok(meta.uid())
    }
    #[cfg(not(unix))]
    {
        Err(PluginError::reject(
            Errno::Einval,
            "rootless id mapping requires a unix host",
        ))
    }
}

// see: cans/plugins/container.md#UID/GID-mapping-isolation
fn passwd_name_for_uid(uid: u32) -> Option<String> {
    let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
    for line in passwd.lines() {
        let mut fields = line.split(':');
        let (name, passwd_uid) = (fields.next(), fields.nth(1));
        if passwd_uid.is_some_and(|passwd_uid| passwd_uid.parse::<u32>() == Ok(uid)) {
            return name.map(str::to_string);
        }
    }
    None
}

// see: cans/plugins/container.md#UID/GID-mapping-isolation
fn parse_subid_file(path: &str, owner: &SubidOwner) -> Result<Option<Range<u32>>, PluginError> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| PluginError::reject(Errno::Enoent, format!("{path} unreadable: {e}")))?;
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
                return Err(PluginError::reject(
                    Errno::Einval,
                    format!("{path} line is not owner:lower:count: {line}"),
                ));
            }
        };
        let lower = lower.parse::<u32>().map_err(|_| {
            PluginError::reject(
                Errno::Einval,
                format!("{path} lower id is not numeric: {line}"),
            )
        })?;
        let count = count.parse::<u32>().map_err(|_| {
            PluginError::reject(
                Errno::Einval,
                format!("{path} count is not numeric: {line}"),
            )
        })?;
        if count == 0 {
            return Err(PluginError::reject(
                Errno::Einval,
                format!("{path} count is zero: {line}"),
            ));
        }
        let end = lower.checked_add(count).ok_or_else(|| {
            PluginError::reject(Errno::Einval, format!("{path} range overflows u32: {line}"))
        })?;
        let owner_match = owner.name.as_deref().is_some_and(|name| user == name)
            || user.parse::<u32>() == Ok(owner.uid);
        if owner_match {
            if matched.is_some() {
                return Err(PluginError::reject(
                    Errno::Einval,
                    format!("{path} carries duplicate entries for uid {}", owner.uid),
                ));
            }
            matched = Some(lower..end);
        }
    }
    Ok(matched)
}

// see: cans/plugins/container.md#UID/GID-mapping-isolation
fn resolve_unprivileged_ranges(
    owner: &SubidOwner,
) -> Result<(Range<u32>, Range<u32>), PluginError> {
    let subuid = parse_subid_file(SUBUID_PATH, owner)?.ok_or_else(|| {
        PluginError::reject(
            Errno::Enoent,
            format!("no subuid range allocated to uid {}", owner.uid),
        )
    })?;
    let subgid = parse_subid_file(SUBGID_PATH, owner)?.ok_or_else(|| {
        PluginError::reject(
            Errno::Enoent,
            format!("no subgid range allocated to uid {}", owner.uid),
        )
    })?;
    Ok((subuid, subgid))
}
