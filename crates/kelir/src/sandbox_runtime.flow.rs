// see: cans/03-sdk-architecture.md#Sandbox-high-level-facade
use std::path::PathBuf;

use kelir_interface::driver_interface::VmConfig;
use kelir_interface::plugin_model::PluginError;
use serde::{Deserialize, Serialize};

use crate::builder_sdk::{Host, KelirBuilder, Unsealed};
use crate::hlist_sdk::{HCons, HNil, NoParams};
use crate::planes::storage_plane::FsReconcileRequest;
use crate::session_lifecycle::{Session, SpawnResult};
use kelir_interface::storage::CommitStats;
use plugin_profile::{ProfileCtx, ProfilePlugin};

pub use crate::profile_schema::{
    ArtifactsProfile, BalloonProfile, BudgetProfile, CatalogProfile, CheckpointProfile,
    ClockProfile, ComputeProfile, DevicesProfile, DriverProfile, DriverProfileTop, EffectiveConfig,
    EmulatedTierClassification, FfiBoundaryProfile, HashingStandard, HostProfile,
    InheritanceLattice, InvariantsProfile, IsolationEnvelopeProfile, LedgerPluginProfile,
    McpInterpositionProfile, McpPluginProfile, MemorySafety, MerkleWalProfile, OtelProfile,
    OutboxDeferralProfile, PhysicalityCheck, PluginsProfile, PolicyPlaneMutability, RejectionMode,
    SessionProfile, ShimProfile, StepDagProfile, StepDagStorage, StorageProfile,
    TaintGovernanceProfile, TelemetryRingProfile, ToolGuardPluginProfile, VirtioRngProfile,
    VirtioShmProfile, VirtioShmRegionProfile, WatchdogProfile, WireProfile,
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
    pub storage: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wire: Option<WireProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<CheckpointProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<HostProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<PluginsProfile>,
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

// see: cans/02-plugin-model.md#Unified-builder
type ProfileHostBuilder = KelirBuilder<Host, Unsealed, (), HNil, HCons<ProfileCtx, HNil>, NoParams>;

pub struct Sandbox {
    builder: Option<ProfileHostBuilder>,
    session: Option<Session<HCons<ProfileCtx, HNil>, NoParams>>,
    profile: SandboxProfile,
    source_path: PathBuf,
}

impl Sandbox {
    pub async fn from_config(path: impl Into<PathBuf>) -> Result<Self, PluginError> {
        let path = path.into();
        let text = tokio::fs::read_to_string(&path)
            .await
            .map_err(PluginError::Io)?;
        let profile = Self::parse_profile(&path, &text)?;
        Self::from_profile(profile, path)
    }

    pub async fn create(opts: SandboxOptions) -> Result<Self, PluginError> {
        let builder = KelirBuilder::new(opts.config)
            .plugin_instance(ProfilePlugin::new())
            .detached(opts.detached);
        Ok(Self {
            builder: Some(builder),
            session: None,
            profile: SandboxProfile::default(),
            source_path: PathBuf::new(),
        })
    }

    fn parse_profile(path: &std::path::Path, text: &str) -> Result<SandboxProfile, PluginError> {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("toml") => toml::from_str(text).map_err(|err| PluginError::Halt(err.to_string())),
            Some("yaml") | Some("yml") => {
                serde_yaml::from_str(text).map_err(|err| PluginError::Halt(err.to_string()))
            }
            Some(other) => Err(PluginError::Halt(format!(
                "Sandbox::from_config: unsupported profile extension `{other}` (expected .toml, .yaml, or .yml)"
            ))),
            None => Err(PluginError::Halt(
                "Sandbox::from_config: profile path has no extension (expected .toml, .yaml, or .yml)"
                    .into(),
            )),
        }
    }

    fn from_profile(profile: SandboxProfile, path: PathBuf) -> Result<Self, PluginError> {
        profile.validate()?;
        let config = profile.to_vm_config();
        let builder = KelirBuilder::new(config).plugin_instance(ProfilePlugin::new());
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

    pub fn session(&self) -> Option<&Session<HCons<ProfileCtx, HNil>, NoParams>> {
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
    pub async fn export_spec(&self, path: impl Into<PathBuf>) -> Result<(), PluginError> {
        let path = path.into();
        let text = match path.extension().and_then(|ext| ext.to_str()) {
            Some("yaml") | Some("yml") => serde_yaml::to_string(&self.profile)
                .map_err(|err| PluginError::Halt(err.to_string()))?,
            _ => {
                toml::to_string(&self.profile).map_err(|err| PluginError::Halt(err.to_string()))?
            }
        };
        tokio::fs::write(path, text)
            .await
            .map_err(PluginError::Io)?;
        Ok(())
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
