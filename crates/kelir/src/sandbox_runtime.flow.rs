// see: cans/03-sdk-architecture.md#Sandbox-high-level-facade
use std::path::PathBuf;

use kelir_interface::checkpoint::ops::{
    BoundaryConfig, CheckpointPlaneConfig, CompactionConfig, StepDagConfig,
};
use kelir_interface::driver_interface::VmConfig;
use kelir_interface::host::ops::{
    CatalogConfig, CgroupConfig, HostPlaneConfig, HostReaperConfig, TelemetryRingConfig,
};
use kelir_interface::plugin_model::PluginError;
use serde::{Deserialize, Serialize};

use crate::builder_sdk::{Host, KelirBuilder, Unsealed};
use crate::hlist_sdk::{HNil, NoParams};
use crate::planes::storage_plane::{FsReconcileRequest, SharedCacheConfig, StoragePlaneConfig};
use crate::planes::terminal_plane::{
    PtyConfig, TerminalSanitizerConfig, TerminalWireframeGridConfig,
};
use crate::planes::wire_plane::{L4Proxy, MtlsConfig, SinkholeConfig, TapFilter, WireConfig};
use crate::session_lifecycle::{Session, SpawnResult};
use kelir_interface::storage::CommitStats;

pub use crate::profile_schema::{
    ArtifactsProfile, BalloonProfile, BudgetProfile, CatalogProfile, CheckpointProfile,
    ClockProfile, ComputeProfile, CowBrokerProfile, DevicesProfile, DriverProfile,
    DriverProfileTop, EffectiveConfig, EmbedMode, EmulatedTierClassification, FfiBoundaryProfile,
    HostProfile, InheritanceLattice, InvariantsProfile, IsolationEnvelopeProfile,
    L4DiversionMtlsProfile, L4DiversionProfile, L4DiversionProxyProfile, MemorySafety,
    OperatorLeaseProfile, OutboxDeferralProfile, PhysicalityCheck, PolicyPlaneMutability,
    RejectionMode, SessionProfile, ShimProfile, StepDagProfile, StepDagStorage, StorageProfile,
    StorageTopProfile, TelemetryRingProfile, VirtioRngProfile, VirtioShmProfile,
    VirtioShmRegionProfile, WatchdogProfile, WireProfile,
};

// see: cans/plugins/profile.md#Session-compute-schema-(profile)
// see: cans/plugins/profile.md#Inversion-of-Control-configuration-router
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SandboxProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invariants: Option<InvariantsProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver: Option<DriverProfileTop>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<StorageTopProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wire: Option<WireProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<CheckpointProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<HostProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    // see: cans/02-plugin-model.md#Facade-blindness
    pub plugins: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct SandboxOptions {
    pub config: VmConfig,
    pub detached: bool,
}

impl SandboxOptions {
    pub fn new(config: VmConfig) -> Self {
        Self {
            config,
            detached: false,
        }
    }

    pub fn detached(mut self, detached: bool) -> Self {
        self.detached = detached;
        self
    }
}

// see: cans/plugins/profile.md#Declarative-profile-execution
pub(crate) fn parse_profile(
    path: &std::path::Path,
    text: &str,
) -> Result<SandboxProfile, PluginError> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("toml") => toml::from_str(text).map_err(|err| PluginError::Halt(err.to_string())),
        Some("yaml") | Some("yml") => {
            serde_yaml::from_str(text).map_err(|err| PluginError::Halt(err.to_string()))
        }
        Some(other) => Err(PluginError::Halt(format!(
            "unsupported profile extension `{other}` (expected .toml, .yaml, or .yml)"
        ))),
        None => Err(PluginError::Halt(
            "profile path has no extension (expected .toml, .yaml, or .yml)".to_string(),
        )),
    }
}

// see: cans/plugins/profile.md#Bidirectional-extraction
pub(crate) async fn write_profile(
    profile: Option<&SandboxProfile>,
    path: PathBuf,
) -> Result<(), PluginError> {
    let Some(profile) = profile else {
        return Err(PluginError::Halt(
            "export_spec: session carries no ingested profile".to_string(),
        ));
    };
    let text = match path.extension().and_then(|ext| ext.to_str()) {
        Some("yaml") | Some("yml") => {
            serde_yaml::to_string(profile).map_err(|err| PluginError::Halt(err.to_string()))?
        }
        _ => toml::to_string(profile).map_err(|err| PluginError::Halt(err.to_string()))?,
    };
    // see: the export write stays here; tokio::fs is L2-clean and no sandbox_runtime.run.rs exists for live code.
    tokio::fs::write(path, text)
        .await
        .map_err(PluginError::Io)?;
    Ok(())
}

// see: cans/05-storage-plane.md#Storage-architecture
pub(crate) fn storage_plane_config(profile: &SandboxProfile) -> Option<StoragePlaneConfig> {
    let storage = profile.storage.as_ref()?;
    let shared_caches = storage
        .extra
        .get("shared_caches")
        .and_then(|value| value.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| {
                    let host_path = entry.get("host_path")?.as_str()?;
                    let guest_mount = entry.get("guest_mount")?.as_str()?;
                    Some(SharedCacheConfig {
                        host_path: PathBuf::from(host_path),
                        guest_mount: PathBuf::from(guest_mount),
                        read_only: entry
                            .get("read_only")
                            .and_then(|value| value.as_bool())
                            .unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Some(StoragePlaneConfig {
        idmap: kelir_engine::planes::storage::IdmapConfig::default(),
        shared_caches,
        encapsulation: kelir_engine::planes::storage::WorkspaceEncapsulation::default(),
        dax_window: storage.dax_window.clone(),
    })
}

// see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
pub(crate) fn wire_plane_config(profile: &SandboxProfile) -> Option<WireConfig> {
    let wire = profile.wire.as_ref()?;
    let l4_proxies = wire
        .l4_diversion
        .as_ref()
        .and_then(|diversion| diversion.proxies.as_ref())
        .map(|proxies| {
            proxies
                .iter()
                .filter_map(|proxy| {
                    let listen_port = proxy.listen_port?;
                    let destination = proxy.destination.clone()?;
                    Some(L4Proxy {
                        listen_port,
                        destination,
                        forward_unix_socket: proxy.forward_unix_socket.clone().map(PathBuf::from),
                        inject_mtls: proxy.inject_mtls.as_ref().map(|mtls| MtlsConfig {
                            cert_path: PathBuf::from(mtls.cert_path.as_deref().unwrap_or_default()),
                            key_path: PathBuf::from(mtls.key_path.as_deref().unwrap_or_default()),
                        }),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Some(WireConfig {
        sinkhole: SinkholeConfig::default(),
        ssh_agent_bridge: false,
        l4_proxies,
        tap_filter: TapFilter::AllowUdpProxied,
        userspace_netstack: wire.userspace_netstack.unwrap_or(false),
        default_egress: wire.default_egress.clone().unwrap_or_default(),
        dial_timeout_ms: 5000,
    })
}

// see: cans/04-terminal-plane.md#Terminal-pipeline
pub(crate) fn pty_config(profile: &SandboxProfile) -> Option<PtyConfig> {
    let terminal = profile.terminal.as_ref()?;
    let pty = terminal.get("pty");
    let sanitizer = terminal.get("sanitizer");
    let wireframe_grid = terminal.get("wireframe_grid");
    Some(PtyConfig {
        cols: u16_of(pty.and_then(|node| node.get("cols")), 80),
        rows: u16_of(pty.and_then(|node| node.get("rows")), 24),
        wireframe_grid: TerminalWireframeGridConfig {
            track_screen_matrix: bool_of(
                wireframe_grid.and_then(|node| node.get("track_screen_matrix")),
            ),
            export_on_trap: bool_of(wireframe_grid.and_then(|node| node.get("export_on_trap"))),
        },
        // see: cans/artifacts/examples/fixtures/kelir.yaml#terminal.stream_engine
        stream_engine: str_of(pty.and_then(|node| node.get("stream_engine"))),
        sanitizer: TerminalSanitizerConfig {
            drop_osc52: bool_of(sanitizer.and_then(|node| node.get("drop_osc52"))),
            strip_title: bool_of(sanitizer.and_then(|node| node.get("strip_title"))),
        },
    })
}

fn u16_of(value: Option<&serde_json::Value>, default: u16) -> u16 {
    value
        .and_then(|value| value.as_u64())
        .and_then(|raw| u16::try_from(raw).ok())
        .unwrap_or(default)
}

fn bool_of(value: Option<&serde_json::Value>) -> bool {
    value.and_then(|value| value.as_bool()).unwrap_or(false)
}

fn str_of(value: Option<&serde_json::Value>) -> String {
    value
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string()
}

// see: cans/01-architecture.md#Host-plane-subsystem
pub(crate) fn host_plane_config(profile: &SandboxProfile) -> Option<HostPlaneConfig> {
    let host = profile.host.as_ref()?;
    let reaper = host.extra.get("reaper");
    let cgroups = host.extra.get("cgroups");
    Some(HostPlaneConfig {
        reaper: HostReaperConfig {
            linux_death_signal: str_of(reaper.and_then(|node| node.get("linux_death_signal"))),
            darwin_kqueue_event: str_of(reaper.and_then(|node| node.get("darwin_kqueue_event"))),
            windows_job_object: bool_of(reaper.and_then(|node| node.get("windows_job_object"))),
        },
        cgroups: CgroupConfig {
            path: str_of(cgroups.and_then(|node| node.get("path"))),
            monitor_pressure: bool_of(cgroups.and_then(|node| node.get("monitor_pressure"))),
            auto_inflate_balloon: bool_of(
                cgroups.and_then(|node| node.get("auto_inflate_balloon")),
            ),
        },
        catalog: CatalogConfig {
            backend: host
                .catalog
                .as_ref()
                .and_then(|catalog| catalog.backend)
                .map(|backend| backend.as_str().to_string())
                .unwrap_or_default(),
            endpoint: host
                .catalog
                .as_ref()
                .and_then(|catalog| catalog.endpoint.clone())
                .unwrap_or_default(),
            schema_tenant_partitioning: host
                .catalog
                .as_ref()
                .and_then(|catalog| catalog.schema_tenant_partitioning)
                .unwrap_or(false),
            fallback_sqlite_path: host
                .catalog
                .as_ref()
                .and_then(|catalog| catalog.fallback_sqlite_path.clone())
                .unwrap_or_default(),
            wal_mode: host
                .catalog
                .as_ref()
                .and_then(|catalog| catalog.wal_mode)
                .unwrap_or(false),
            busy_timeout_ms: host
                .catalog
                .as_ref()
                .and_then(|catalog| catalog.busy_timeout_ms)
                .unwrap_or(0),
            lease_ttl_seconds: host
                .catalog
                .as_ref()
                .and_then(|catalog| catalog.lease_ttl_seconds)
                .unwrap_or(0),
            prune_unreferenced_cas_blobs: host
                .catalog
                .as_ref()
                .and_then(|catalog| catalog.prune_unreferenced_cas_blobs)
                .unwrap_or(false),
        },
        telemetry_ring: TelemetryRingConfig {
            feature_extraction: host
                .telemetry_ring
                .as_ref()
                .and_then(|ring| ring.feature_extraction)
                .unwrap_or(false),
            sliding_window_depth: host
                .telemetry_ring
                .as_ref()
                .and_then(|ring| ring.sliding_window_depth)
                .map(|depth| depth as u32)
                .unwrap_or(0),
            export_feature_vector: host
                .telemetry_ring
                .as_ref()
                .and_then(|ring| ring.export_feature_vector)
                .unwrap_or(false),
        },
    })
}

// see: cans/08-lifecycle.md#Native-checkpoint-plane-(kelir-engine::checkpoint)
pub(crate) fn checkpoint_plane_config(profile: &SandboxProfile) -> Option<CheckpointPlaneConfig> {
    let checkpoint = profile.checkpoint.as_ref()?;
    let lineage = checkpoint.extra.get("lineage");
    let boundaries = checkpoint.extra.get("boundaries");
    Some(CheckpointPlaneConfig {
        step_dag: StepDagConfig {
            enabled: checkpoint
                .step_dag
                .as_ref()
                .and_then(|dag| dag.enabled)
                .unwrap_or(false),
            require_signed_attribution: checkpoint
                .step_dag
                .as_ref()
                .and_then(|dag| dag.require_signed_attribution)
                .unwrap_or(false),
            require_separation_of_duties: checkpoint
                .step_dag
                .as_ref()
                .and_then(|dag| dag.require_separation_of_duties)
                .unwrap_or(false),
            storage: checkpoint
                .step_dag
                .as_ref()
                .and_then(|dag| dag.storage)
                .map(|storage| storage.as_str().to_string())
                .unwrap_or_default(),
        },
        compaction: CompactionConfig {
            unbounded_depth: bool_of(lineage.and_then(|node| node.get("unbounded_depth"))),
            lazy_cow_sharing: bool_of(lineage.and_then(|node| node.get("lazy_cow_sharing"))),
            compaction_on_export: bool_of(
                lineage.and_then(|node| node.get("compaction_on_export")),
            ),
        },
        snapshot_boundaries: BoundaryConfig {
            sentinel_path: str_of(boundaries.and_then(|node| node.get("sentinel_path"))),
            detect_pty_quiescence: bool_of(
                boundaries.and_then(|node| node.get("detect_pty_quiescence")),
            ),
            pre_checkpoint_dentry_trim: bool_of(
                boundaries.and_then(|node| node.get("pre_checkpoint_dentry_trim")),
            ),
        },
    })
}

// see: cans/02-plugin-model.md#Unified-builder
type HostBuilder = KelirBuilder<Host, Unsealed, (), HNil, HNil, NoParams>;

pub struct Sandbox {
    builder: Option<HostBuilder>,
    session: Option<Session<HNil, NoParams>>,
    profile: SandboxProfile,
    source_path: PathBuf,
}

impl Sandbox {
    pub async fn from_config(path: impl Into<PathBuf>) -> Result<Self, PluginError> {
        let path = path.into();
        let text = tokio::fs::read_to_string(&path)
            .await
            .map_err(PluginError::Io)?;
        let profile = parse_profile(&path, &text)?;
        Self::from_profile(profile, path)
    }

    pub async fn create(opts: SandboxOptions) -> Result<Self, PluginError> {
        let builder = KelirBuilder::new(opts.config).detached(opts.detached);
        Ok(Self {
            builder: Some(builder),
            session: None,
            profile: SandboxProfile::default(),
            source_path: PathBuf::new(),
        })
    }

    fn from_profile(profile: SandboxProfile, path: PathBuf) -> Result<Self, PluginError> {
        profile.validate()?;
        let config = profile.to_vm_config();
        let builder = KelirBuilder::new(config);
        Ok(Self {
            builder: Some(builder),
            session: None,
            profile,
            source_path: path,
        })
    }

    pub fn profile(&self) -> &SandboxProfile {
        &self.profile
    }

    pub fn source_path(&self) -> &PathBuf {
        &self.source_path
    }

    pub fn session(&self) -> Option<&Session<HNil, NoParams>> {
        self.session.as_ref()
    }

    pub fn is_detached(&self) -> bool {
        if let Some(builder) = &self.builder {
            return builder.is_detached();
        }
        if let Some(session) = &self.session {
            return session.is_detached();
        }
        false
    }

    pub async fn boot(mut self) -> Result<Self, PluginError> {
        let Some(builder) = self.builder.take() else {
            return Err(PluginError::Halt("Sandbox::boot called twice".into()));
        };
        let session = builder.seal()?.boot().await?;
        self.session = Some(session);
        Ok(self)
    }

    // see: cans/03-sdk-architecture.md#Sandbox-high-level-facade
    pub async fn exec(&mut self, binary: &str, argv: &[&str]) -> Result<SpawnResult, PluginError> {
        let Some(session) = self.session.as_ref() else {
            return Err(PluginError::Halt("Sandbox::exec before boot".into()));
        };
        session.spawn(binary, argv, None).await
    }

    pub async fn rollback(&mut self, lineage: crate::PidLineage) -> Result<(), PluginError> {
        let Some(session) = self.session.as_mut() else {
            return Err(PluginError::Halt("Sandbox::rollback before boot".into()));
        };
        session.rollback(lineage).await
    }

    pub async fn suspend(
        &mut self,
        reason: kelir_interface::driver_interface::SuspendReason,
    ) -> Result<(), PluginError> {
        let Some(session) = self.session.as_mut() else {
            return Err(PluginError::Halt("Sandbox::suspend before boot".into()));
        };
        session.suspend(reason).await
    }

    // see: cans/08-lifecycle.md#Suspend-and-resume
    pub async fn resume(&mut self) -> Result<(), PluginError> {
        let Some(session) = self.session.as_mut() else {
            return Err(PluginError::Halt("Sandbox::resume before boot".into()));
        };
        session.resume().await
    }

    pub fn reconcile(&self, req: FsReconcileRequest) -> Result<CommitStats, PluginError> {
        let Some(session) = self.session.as_ref() else {
            return Err(PluginError::Halt("Sandbox::reconcile before boot".into()));
        };
        session.core().storage.reconcile(req)
    }

    // see: cans/plugins/profile.md#Bidirectional-extraction
    // TODO (L2) [profile_ingestion]: overlay programmatic plane descriptors over the ingested profile before export.; slop: post-ingestion descriptor overrides silently absent from the export
    pub async fn export_spec(&self, path: impl Into<PathBuf>) -> Result<(), PluginError> {
        write_profile(Some(&self.profile), path.into()).await
    }

    pub async fn stop(&mut self) -> Result<(), PluginError> {
        let Some(session) = self.session.as_mut() else {
            return Err(PluginError::Halt("Sandbox::stop before boot".into()));
        };
        session.stop().await
    }

    // see: cans/08-lifecycle.md#Pre-teardown-flush
    pub async fn stop_with_timeout(
        &mut self,
        timeout: std::time::Duration,
    ) -> Result<(), PluginError> {
        let Some(session) = self.session.as_mut() else {
            return Err(PluginError::Halt(
                "Sandbox::stop_with_timeout before boot".into(),
            ));
        };
        session.stop_with_timeout(timeout).await
    }

    // see: cans/08-lifecycle.md#Pre-teardown-flush
    pub async fn terminate(&mut self) -> Result<(), PluginError> {
        let Some(session) = self.session.as_mut() else {
            return Err(PluginError::Halt("Sandbox::terminate before boot".into()));
        };
        session.kill()
    }
}
