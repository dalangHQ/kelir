// see: cans/03-sdk-architecture.md#Sandbox-high-level-facade
use std::path::PathBuf;

use kelir_interface::driver::{VmConfig, default_vm_config};
use kelir_interface::plugin::PluginError;
use serde::{Deserialize, Serialize};

use crate::builder::{KelirBuilder, Unsealed};
use crate::hlist::{HCons, HNil, NoParams};
use crate::planes::storage::FsReconcileRequest;
use crate::session::{Session, SpawnBuilder};
use kelir_core::planes::storage::reconciler::CommitStats;
use plugin_profile::ProfilePlugin;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SandboxProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionProfile>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compute: Option<ComputeProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<ArtifactsProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock: Option<ClockProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watchdog: Option<WatchdogProfile>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComputeProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcpus: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_mb: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_size_kb: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArtifactsProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kernel_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rootfs_erofs_path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClockProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tsc_locked: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deterministic_clock: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_clocksource: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preemption_grace_ms: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WatchdogProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

impl SandboxProfile {
    pub fn to_vm_config(&self) -> VmConfig {
        let session = self.session.as_ref();
        let compute = session.and_then(|s| s.compute.as_ref());
        let artifacts = session.and_then(|s| s.artifacts.as_ref());
        let clock = session.and_then(|s| s.clock.as_ref());
        let watchdog = session.and_then(|s| s.watchdog.as_ref());
        let mut config = default_vm_config(
            compute.and_then(|c| c.vcpus),
            compute.and_then(|c| c.memory_mb),
            artifacts.and_then(|a| a.kernel_path.clone()),
            artifacts.and_then(|a| a.rootfs_erofs_path.clone()),
            clock.and_then(|c| c.tsc_locked),
            watchdog.and_then(|w| w.enabled),
        );
        if let Some(page_size_kb) = compute.and_then(|c| c.page_size_kb) {
            config.page_size_kb = page_size_kb;
        }
        if let Some(deterministic_clock) = clock.and_then(|c| c.deterministic_clock) {
            config.deterministic_clock = deterministic_clock;
        }
        if let Some(pinned) = clock.and_then(|c| c.pinned_clocksource.clone()) {
            config.pinned_clocksource = Some(pinned);
        }
        if let Some(grace) = clock.and_then(|c| c.preemption_grace_ms) {
            config.preemption_grace_ms = grace;
        }
        config
    }
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

pub struct Sandbox {
    builder: Option<KelirBuilder<Unsealed, HCons<(), HNil>, NoParams>>,
    session: Option<Session<HCons<(), HNil>, NoParams>>,
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
            .plugin(ProfilePlugin::new())
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
        let config = profile.to_vm_config();
        let builder = KelirBuilder::new(config).plugin(ProfilePlugin::new());
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

    pub fn session(&self) -> Option<&Session<HCons<(), HNil>, NoParams>> {
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
        let session = builder.boot().await?;
        self.session = Some(session);
        Ok(self)
    }

    pub fn exec(&mut self, binary: &str) -> SpawnBuilder {
        match &self.session {
            Some(session) => session.spawn(binary),
            None => SpawnBuilder::default().args([binary]),
        }
    }

    pub async fn rollback(&mut self, lineage: crate::PidLineage) -> Result<(), PluginError> {
        let Some(session) = self.session.as_mut() else {
            return Err(PluginError::Halt("Sandbox::rollback before boot".into()));
        };
        session.rollback(lineage).await
    }

    pub fn suspend(
        &mut self,
        reason: kelir_interface::driver::SuspendReason,
    ) -> Result<(), PluginError> {
        let Some(session) = self.session.as_mut() else {
            return Err(PluginError::Halt("Sandbox::suspend before boot".into()));
        };
        session.suspend(reason)
    }

    pub fn reconcile(&self, req: FsReconcileRequest) -> Result<CommitStats, PluginError> {
        let Some(session) = self.session.as_ref() else {
            return Err(PluginError::Halt("Sandbox::reconcile before boot".into()));
        };
        session.core().storage.reconcile(req)
    }

    pub async fn export_spec(&self, path: impl Into<PathBuf>) -> Result<(), PluginError> {
        let text =
            toml::to_string(&self.profile).map_err(|err| PluginError::Halt(err.to_string()))?;
        tokio::fs::write(path.into(), text)
            .await
            .map_err(PluginError::Io)?;
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<(), PluginError> {
        let Some(session) = self.session.as_mut() else {
            return Err(PluginError::Halt("Sandbox::stop before boot".into()));
        };
        session.stop()
    }

    pub async fn terminate(&mut self) -> Result<(), PluginError> {
        self.stop().await
    }
}
