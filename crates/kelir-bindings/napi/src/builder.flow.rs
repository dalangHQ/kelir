// see: cans/03-sdk-architecture.md#TypeScript-SDK
use std::sync::Arc;
use std::time::Instant;

use napi::bindgen_prelude::Object;
use napi::{Error, Result, Status};
use napi_derive::napi;

use kelir_engine::Session;
use kelir_interface::driver_interface::{VmConfig, default_vm_config};
use kelir_interface::plugin_model::PluginRegistration;

use crate::fusion_plugin::{
    FusedTrampolineDescriptor, TrampolineTable, attach_runs, convert_descriptor,
};
use crate::handles::NativeSessionHandle;
use crate::handles::halt;
use crate::storage_bridge::{self, storage_config};

// see: cans/03-sdk-architecture.md#Session-control-surface
#[napi(object)]
pub struct NativeTerminalConfig {
    pub cols: Option<u16>,
    pub rows: Option<u16>,
    pub drop_osc52: Option<bool>,
    pub strip_title: Option<bool>,
    pub token_shield_max_bytes_total: Option<f64>,
    pub token_shield_max_lines_total: Option<f64>,
    pub token_shield_clamp_strategy: Option<String>,
    pub stream_engine: Option<String>,
}

// see: cans/03-sdk-architecture.md#Session-control-surface
#[napi(object)]
pub struct NativeStorageConfig {
    pub idmap_guest_uid: Option<f64>,
    pub idmap_host_uid: Option<f64>,
    pub shared_caches: Option<Vec<String>>,
    pub lock_broker: Option<bool>,
    pub synthetic_files: Option<Vec<NativeSyntheticFile>>,
    pub mask_read: Option<Vec<NativeMaskRule>>,
    pub encapsulation: Option<String>,
    pub scan_engine: Option<String>,
    pub carryover_bytes: Option<f64>,
}

// see: cans/05-storage-plane.md#Workspace-commit
#[napi(object)]
pub struct NativeSyntheticFile {
    pub path: String,
    pub content: String,
    pub mode: Option<f64>,
}

// see: cans/05-storage-plane.md#Workspace-commit
#[napi(object)]
pub struct NativeMaskRule {
    pub glob: String,
    pub pattern: String,
    pub replace: String,
}

// see: cans/07-wire-plane.md#DNS-policy-filter
#[napi(object)]
pub struct NativeVaultDescriptor {
    pub name: String,
    pub kind: String,
    pub audience: String,
    pub placeholder: String,
}

// see: cans/07-wire-plane.md#Port-forwarding
#[napi(object)]
pub struct NativeL4Proxy {
    pub listen_port: u16,
    pub destination: String,
    pub inject_mtls_cert_path: Option<String>,
    pub inject_mtls_key_path: Option<String>,
}

// see: cans/07-wire-plane.md#DNS-policy-filter
#[napi(object)]
pub struct NativeWireConfig {
    pub sinkhole_dns_except: Option<Vec<String>>,
    pub sinkhole_drop_all: Option<bool>,
    pub ssh_agent_bridge: Option<bool>,
    pub vaults: Option<Vec<NativeVaultDescriptor>>,
    pub l4_proxies: Option<Vec<NativeL4Proxy>>,
    pub tap_filter: Option<String>,
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
#[derive(Default)]
#[napi(object)]
pub struct NativeKelirOptions {
    pub vcpus: Option<u8>,
    pub memory_mb: Option<u32>,
    pub kernel: Option<String>,
    pub rootfs_erofs: Option<String>,
    pub tsc_locked: Option<bool>,
    pub enable_watchdog: Option<bool>,
    pub driver: Option<String>,
}

#[napi]
pub struct NativeKelirBuilder {
    config: VmConfig,
    driver_name: String,
    plugins: Vec<PluginRegistration>,
    fused: Option<FusedTrampolineDescriptor>,
    trampolines: TrampolineTable,
    terminal_cfg: Option<NativeTerminalConfig>,
    storage_cfg: Option<NativeStorageConfig>,
    wire_cfg: Option<NativeWireConfig>,
}

#[napi]
impl NativeKelirBuilder {
    #[napi(constructor)]
    pub fn new(options: Option<NativeKelirOptions>) -> Self {
        let options = options.unwrap_or_default();
        let mut config = default_vm_config(
            options.vcpus,
            options.memory_mb,
            options.kernel,
            options.rootfs_erofs,
            options.tsc_locked,
            options.enable_watchdog,
        );
        // see: cans/01-architecture.md#Driver-backends
        config.driver_policy.driver = options.driver.clone();
        Self {
            config,
            driver_name: options.driver.unwrap_or_else(|| "auto".to_string()),
            plugins: Vec::new(),
            fused: None,
            trampolines: TrampolineTable::default(),
            terminal_cfg: None,
            storage_cfg: None,
            wire_cfg: None,
        }
    }

    // see: cans/04-terminal-plane.md#Terminal-pipeline
    // TODO (L3) [bindings_ts]: prompt rules and the token-shield clamp callback stay TypeScript-side until threadsafe-function bridging lands.; slop: callbacks stay TS-side, silently dropped
    #[napi]
    pub fn set_terminal_config(&mut self, config: NativeTerminalConfig) {
        self.terminal_cfg = Some(config);
    }

    // see: cans/05-storage-plane.md#Workspace-commit
    // TODO (L3) [bindings_ts]: snoop on_write callbacks stay TypeScript-side until threadsafe-function bridging lands.; slop: snoop callbacks stay TS-side, writes unobserved
    #[napi]
    pub fn set_storage_config(&mut self, config: NativeStorageConfig) {
        self.storage_cfg = Some(config);
    }

    // see: cans/07-wire-plane.md#DNS-policy-filter
    #[napi]
    pub fn set_wire_config(&mut self, config: NativeWireConfig) {
        self.wire_cfg = Some(config);
    }

    #[napi]
    pub fn add_plugin(&mut self, name: String, enabled: Option<bool>) -> Result<()> {
        if name.trim().is_empty() {
            return Err(Error::new(
                Status::InvalidArg,
                "add_plugin: plugin name must not be empty".to_string(),
            ));
        }
        if self.plugins.iter().any(|plugin| plugin.name == name) {
            return Ok(());
        }
        self.plugins.push(PluginRegistration {
            name,
            enabled: enabled.unwrap_or(true),
        });
        Ok(())
    }

    #[napi]
    pub fn fuse_hooks(&mut self, rings: Vec<Object>, runs: Option<Vec<Object>>) -> Result<()> {
        if self.fused.is_some() {
            return Err(Error::from_reason(
                "fuse_hooks: the trampoline descriptor is frozen at seal".to_string(),
            ));
        }
        let mut descriptor = convert_descriptor(rings)?;
        if let Some(runs) = runs {
            attach_runs(&mut descriptor, runs)?;
        }
        self.trampolines =
            TrampolineTable::from_descriptor(&descriptor).map_err(Error::from_reason)?;
        self.fused = Some(descriptor);
        Ok(())
    }

    #[napi]
    pub fn trampoline_entry_count(&self) -> u32 {
        self.trampolines.entry_count()
    }

    #[napi]
    pub fn boot(&self) -> Result<NativeSessionHandle> {
        let Some(descriptor) = &self.fused else {
            return Err(Error::from_reason(
                "boot: fuse_hooks must run before boot".to_string(),
            ));
        };
        if descriptor.rings.len() as u32 != self.trampolines.entry_count() {
            return Err(Error::from_reason(
                "boot: the trampoline table is not one entry per ring event".to_string(),
            ));
        }
        for chain in &descriptor.rings {
            for hook in &chain.hooks {
                if let Some(plugin_name) = &hook.plugin_name {
                    match self
                        .plugins
                        .iter()
                        .find(|plugin| &plugin.name == plugin_name)
                    {
                        Some(registration) if registration.enabled => {}
                        Some(_) => {
                            return Err(Error::from_reason(format!(
                                "boot: fused ring {} carries hooks of disabled plugin `{plugin_name}`",
                                chain.ring.as_tag()
                            )));
                        }
                        None => {
                            return Err(Error::from_reason(format!(
                                "boot: fused ring {} carries hooks of unregistered plugin `{plugin_name}`",
                                chain.ring.as_tag()
                            )));
                        }
                    }
                }
            }
        }
        if let Some(reason) = self.config.missing_boot_path() {
            return Err(Error::new(Status::InvalidArg, reason.to_string()));
        }
        // TODO (L3) [bt_03_typescript_fused_trampoline_single_crossing_chained_hooks]: rendezvous the JS return value into a Verdict and marshal the ring event payload across the threadsafe boundary; slop: JS hook runs fire-and-forget, its verdict never gates dispatch
        let plugins: Vec<PluginRegistration> = self.plugins.clone();
        let plane_wiring = self.plane_wiring()?;
        let synthetic_files = self
            .storage_cfg
            .as_ref()
            .map(storage_bridge::synthetic_files)
            .transpose()?
            .unwrap_or_default();
        let started = Instant::now();
        let session = Session::boot_with_plugins(
            self.config.clone(),
            &plugins,
            Some(descriptor),
            synthetic_files,
            plane_wiring,
        )
        .map_err(halt)?;
        // TODO (L3) [hypervisor_driver]: re-source driver_name from the booted core session once it exposes the resolved driver.; slop: driver name stale, wrong backend reported
        NativeSessionHandle::from_session(
            session,
            self.driver_name.clone(),
            started.elapsed().as_secs_f64() * 1000.0,
        )
    }
}

impl Default for NativeKelirBuilder {
    fn default() -> Self {
        Self::new(None)
    }
}

// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
impl NativeKelirBuilder {
    // TODO (L3) [bindings_ts]: token-shield clamps, prompt rules, snoop callbacks, and mtls injection ride threadsafe functions into the planes.; slop: plane wiring drops clamps, snoop, mtls
    fn plane_wiring(&self) -> Result<kelir_engine::session_lifecycle::PlaneWiring> {
        let terminal =
            self.terminal_cfg
                .as_ref()
                .map(|cfg| kelir_interface::terminal::TerminalWiring {
                    cols: cfg.cols.unwrap_or(80),
                    rows: cfg.rows.unwrap_or(24),
                    drop_osc52: cfg.drop_osc52.unwrap_or(false),
                    strip_title: cfg.strip_title.unwrap_or(false),
                    token_shield: None,
                    prompts: Vec::new(),
                });
        let wire = self
            .wire_cfg
            .as_ref()
            .map(|cfg| kelir_interface::wire::net::WireWiring {
                sinkhole_allowlist: cfg.sinkhole_dns_except.clone().unwrap_or_default(),
                sinkhole_drop_all: cfg.sinkhole_drop_all.unwrap_or(false),
                ssh_agent_bridge: cfg.ssh_agent_bridge.unwrap_or(false),
                vault_placeholders: cfg
                    .vaults
                    .as_ref()
                    .map(|vaults| {
                        vaults
                            .iter()
                            .map(|vault| vault.placeholder.clone())
                            .collect()
                    })
                    .unwrap_or_default(),
                l4_proxies: cfg
                    .l4_proxies
                    .as_ref()
                    .map(|proxies| {
                        proxies
                            .iter()
                            .map(|proxy| (proxy.listen_port, proxy.destination.clone()))
                            .collect()
                    })
                    .unwrap_or_default(),
                allow_udp_proxied: cfg.tap_filter.as_deref() == Some("AllowUdpProxied"),
            });
        let storage = match &self.storage_cfg {
            Some(cfg) => storage_config(cfg)?,
            None => kelir_engine::planes::storage::StorageConfig::default(),
        };
        // see: cans/01-architecture.md#Host-runtime
        let metrics = Arc::new(kelir_otlp::registry_otlp::MetricRegistry::new());
        let dns_ops: Arc<dyn kelir_interface::wire::dns::DnsOps> =
            Arc::new(kelir_wire::RealDnsOps::new());
        // TODO (L3) [composition_root]: bind RealFuseOps once Session shares Arc<PipelineEngine>; slop: FUSE dispatch unreachable from the session, dispatcher built only in tests
        // TODO (L3) [composition_root]: bind the kelir-storage Reconciler once the host broker journal anchors broker_dir; slop: StoragePlane::reconcile fails closed without it, terminate flush never commits
        Ok(kelir_engine::session_lifecycle::PlaneWiring {
            terminal,
            terminal_ops: Some(Arc::new(kelir_terminal::RealTerminalOps::new(
                metrics.clone(),
            ))),
            checkpoint_ops: Some(Arc::new(kelir_checkpoint::RealCheckpointOps::new(
                metrics.clone(),
            ))),
            host_ops: Some(Arc::new(kelir_host::RealHostOps::new(metrics.clone()))),
            net_ops: Some(Arc::new(kelir_wire::RealNetOps::new(Arc::clone(&dns_ops)))),
            dns_ops: Some(dns_ops),
            udp_nat_ops: Some(Arc::new(kelir_wire::RealUdpNatOps::new())),
            l4_ops: Some(Arc::new(kelir_wire::RealL4DivertOps::new())),
            storage_io: Some(Arc::new(kelir_storage_io::RealStorageIo)),
            reconciler: None,
            fuse_ops: None,
            wire,
            storage,
        })
    }
}
