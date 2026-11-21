// see: cans/03-sdk-architecture.md#TypeScript-SDK
use std::sync::Arc;
use std::time::Instant;

use napi::bindgen_prelude::Object;
use napi::{Error, Result, Status};
use napi_derive::napi;

use kelir_engine::Session;
use kelir_interface::driver_interface::VmConfig;
use kelir_interface::plugin_model::PluginRegistration;

use crate::fusion_plugin::{
    FusedTrampolineDescriptor, TrampolineTable, attach_runs, convert_descriptor,
};
use crate::handles::NativeSessionHandle;
use crate::handles::halt;
use crate::storage_bridge::storage_config;

// see: cans/03-sdk-architecture.md#Session-control-surface
#[napi(object)]
pub struct NativeTerminalConfig {
    pub cols: Option<u16>,
    pub rows: Option<u16>,
    pub wireframe_grid: Option<NativeTerminalWireframeGrid>,
    pub sanitizer: Option<NativeTerminalSanitizer>,
    pub stream_engine: Option<String>,
}

// see: cans/05-storage-plane.md#Storage-architecture
#[napi(object)]
pub struct NativeSharedCache {
    pub host_path: String,
    pub guest_mount: String,
    pub read_only: Option<bool>,
}

// see: cans/03-sdk-architecture.md#Session-control-surface
#[napi(object)]
pub struct NativeStorageConfig {
    pub idmap_guest_uid: Option<f64>,
    pub idmap_host_uid: Option<f64>,
    pub shared_caches: Option<Vec<NativeSharedCache>>,
    pub lock_broker: Option<bool>,
    pub encapsulation: Option<String>,
    pub blocking_wait_timeout_ms: Option<u32>,
}

// see: cans/04-terminal-plane.md#Headless-VT100-wireframe-grid
#[napi(object)]
pub struct NativeTerminalWireframeGrid {
    pub track_screen_matrix: Option<bool>,
    pub export_on_trap: Option<bool>,
}

// see: cans/04-terminal-plane.md#Terminal-pipeline
#[napi(object)]
pub struct NativeTerminalSanitizer {
    pub drop_osc52: Option<bool>,
    pub strip_title: Option<bool>,
}

// see: cans/07-wire-plane.md#Port-forwarding
#[napi(object)]
pub struct NativeL4Proxy {
    pub listen_port: u16,
    pub destination: String,
    pub forward_unix_socket: Option<String>,
    pub inject_mtls_cert_path: Option<String>,
    pub inject_mtls_key_path: Option<String>,
}

// see: cans/07-wire-plane.md#Sinkhole-rules
#[napi(object)]
pub struct NativeSinkholeRule {
    pub host: String,
    pub allow: Option<bool>,
}

// see: cans/07-wire-plane.md#DNS-policy-filter
#[napi(object)]
pub struct NativeSinkholeConfig {
    pub rules: Option<Vec<NativeSinkholeRule>>,
}

// see: cans/07-wire-plane.md#DNS-policy-filter
#[napi(object)]
pub struct NativeWireConfig {
    pub sinkhole: Option<NativeSinkholeConfig>,
    pub ssh_agent_bridge: Option<bool>,
    pub dial_timeout_ms: Option<u32>,
    pub l4_proxies: Option<Vec<NativeL4Proxy>>,
    pub tap_filter: Option<String>,
    pub block_abstract_unix_sockets: Option<bool>,
    pub dns_to_ip_pinning: Option<bool>,
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
    pub mask_proc_environ: Option<bool>,
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
    wire_dns_to_ip_pinning: Option<bool>,
}

#[napi]
impl NativeKelirBuilder {
    #[napi(constructor)]
    pub fn new(options: Option<NativeKelirOptions>) -> Self {
        let options = options.unwrap_or_default();
        let mut config = VmConfig::default();
        if let Some(vcpus) = options.vcpus {
            config.vcpus = vcpus;
        }
        if let Some(memory_mb) = options.memory_mb {
            config.memory_mb = memory_mb;
        }
        if let Some(kernel) = options.kernel {
            config.kernel_path = kernel.into();
        }
        if let Some(rootfs) = options.rootfs_erofs {
            config.rootfs_erofs_path = rootfs.into();
        }
        if let Some(tsc_locked) = options.tsc_locked {
            config.tsc_locked = tsc_locked;
        }
        if let Some(enable_watchdog) = options.enable_watchdog {
            config.enable_watchdog = enable_watchdog;
        }
        if let Some(mask_proc_environ) = options.mask_proc_environ {
            config.mask_proc_environ = mask_proc_environ;
        }
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
            wire_dns_to_ip_pinning: None,
        }
    }

    // see: cans/04-terminal-plane.md#Terminal-pipeline
    // TODO: [bindings_ts]: prompt rules and the token-shield clamp callback stay TypeScript-side until threadsafe-function bridging lands.; slop: callbacks stay TS-side, silently dropped (deferred to L3)
    #[napi]
    pub fn set_terminal_config(&mut self, config: NativeTerminalConfig) {
        self.terminal_cfg = Some(config);
    }

    // see: cans/05-storage-plane.md#Workspace-commit
    // TODO: [bindings_ts]: snoop on_write callbacks stay TypeScript-side until threadsafe-function bridging lands.; slop: snoop callbacks stay TS-side, writes unobserved (deferred to L3)
    #[napi]
    pub fn set_storage_config(&mut self, config: NativeStorageConfig) {
        self.storage_cfg = Some(config);
    }

    // see: cans/07-wire-plane.md#DNS-policy-filter
    #[napi]
    pub fn set_wire_config(&mut self, config: NativeWireConfig) {
        self.wire_dns_to_ip_pinning = config.dns_to_ip_pinning;
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
        // TODO: [bt_03_typescript_fused_trampoline_single_crossing_chained_hooks]: rendezvous the JS return value into a Verdict and marshal the ring event payload across the threadsafe boundary; slop: JS hook runs fire-and-forget, its verdict never gates dispatch (deferred to L3)
        let plugins: Vec<PluginRegistration> = self.plugins.clone();
        let plane_wiring = self.plane_wiring()?;
        let started = Instant::now();
        let session = Session::boot_with_plugins(
            self.config.clone(),
            &plugins,
            Some(descriptor),
            plane_wiring,
        )
        .map_err(halt)?;
        // TODO: [hypervisor_driver]: re-source driver_name from the booted core session once it exposes the resolved driver.; slop: driver name stale, wrong backend reported (deferred to L3)
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
    // TODO: [bindings_ts]: snoop callbacks and mtls injection ride threadsafe functions into the planes.; slop: plane wiring drops snoop, mtls (deferred to L3)
    fn plane_wiring(&self) -> Result<kelir_engine::session_lifecycle::PlaneWiring> {
        // TODO: [bindings_ts]: forward wireframe_grid flags and stream_engine into TerminalWiring once it carries them; slop: grid tracking and engine selection never reach the terminal plane. (deferred to L3)
        let terminal =
            self.terminal_cfg
                .as_ref()
                .map(|cfg| kelir_interface::terminal::TerminalWiring {
                    cols: cfg.cols.unwrap_or(80),
                    rows: cfg.rows.unwrap_or(24),
                    drop_osc52: cfg
                        .sanitizer
                        .as_ref()
                        .and_then(|sanitizer| sanitizer.drop_osc52)
                        .unwrap_or(false),
                    strip_title: cfg
                        .sanitizer
                        .as_ref()
                        .and_then(|sanitizer| sanitizer.strip_title)
                        .unwrap_or(false),
                });
        let wire = self.wire_cfg.as_ref().map(|cfg| {
            let sinkhole = kelir_interface::wire::net::SinkholeConfig {
                rules: cfg
                    .sinkhole
                    .as_ref()
                    .and_then(|sinkhole| sinkhole.rules.as_ref())
                    .map(|rules| {
                        rules
                            .iter()
                            .map(|rule| kelir_interface::wire::net::SinkholeRule {
                                host: rule.host.clone(),
                                allow: rule.allow.unwrap_or(false),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            };
            kelir_interface::wire::net::WireWiring {
                sinkhole,
                ssh_agent_bridge: cfg.ssh_agent_bridge.unwrap_or(false),
                dial_timeout_ms: u64::from(cfg.dial_timeout_ms.unwrap_or(5000)),
                dns_to_ip_pinning: self.wire_dns_to_ip_pinning.unwrap_or(false),
                connect_redirect: None,
                dns_redirect: None,
                l4_proxies: cfg
                    .l4_proxies
                    .as_ref()
                    .map(|proxies| {
                        proxies
                            .iter()
                            .filter(|proxy| proxy.forward_unix_socket.is_none())
                            .map(|proxy| (proxy.listen_port, proxy.destination.clone()))
                            .collect()
                    })
                    .unwrap_or_default(),
                allow_udp_proxied: cfg.tap_filter.as_deref() == Some("AllowUdpProxied"),
                userspace_netstack: false,
                default_egress: String::new(),
                block_abstract_unix_sockets: cfg.block_abstract_unix_sockets.unwrap_or(false),
                l4_unix_diversions: cfg
                    .l4_proxies
                    .as_ref()
                    .map(|proxies| {
                        proxies
                            .iter()
                            .filter_map(|proxy| {
                                proxy
                                    .forward_unix_socket
                                    .as_ref()
                                    .map(|socket| (proxy.listen_port, socket.as_str().into()))
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            }
        });
        let storage = match &self.storage_cfg {
            Some(cfg) => storage_config(cfg)?,
            None => kelir_engine::planes::storage::StorageConfig::default(),
        };
        // TODO: [plugin_engine]: per_exec_cgroup and deny_abi_compat are ShimProfile fields with no native shim-config surface on this builder; slop: seccomp fields accepted by the profile schema, never reach the shim (deferred to L3)
        // see: cans/01-architecture.md#Host-runtime
        let metrics = Arc::new(kelir_interface::telemetry::MetricRegistry::new());
        let dns_ops: Arc<dyn kelir_interface::wire::dns::DnsOps> = kelir_wire::dns_ops(
            std::sync::Arc::new(kelir_engine::dispatch_plugin::PipelineEngine::new()),
        );
        // TODO: [composition_root]: bind RealFuseOps once Session shares Arc<PipelineEngine>; slop: FUSE dispatch unreachable from the session, dispatcher built only in tests (deferred to L3)
        // TODO: [composition_root]: bind the kelir-storage Reconciler once the host broker journal anchors broker_dir; slop: StoragePlane::reconcile fails closed without it, terminate flush never commits (deferred to L3)
        Ok(kelir_engine::session_lifecycle::PlaneWiring {
            terminal,
            terminal_ops: Some(kelir_terminal::terminal_ops(metrics.clone())),
            checkpoint_ops: Some(kelir_checkpoint::checkpoint_ops(metrics.clone())),
            checkpoint_config: None,
            host_ops: Some(kelir_host::host_ops(metrics.clone())),
            host_config: None,
            net_ops: Some(kelir_wire::net_ops(
                std::sync::Arc::new(kelir_engine::dispatch_plugin::PipelineEngine::new()),
                Arc::clone(&dns_ops),
            )),
            dns_ops: Some(dns_ops),
            udp_nat_ops: Some(kelir_wire::udp_nat_ops()),
            l4_ops: Some(kelir_wire::l4_ops()),
            storage_io: Some(kelir_storage_io::storage_io()),
            reconciler: None,
            fuse_ops: None,
            wire,
            storage,
        })
    }
}
