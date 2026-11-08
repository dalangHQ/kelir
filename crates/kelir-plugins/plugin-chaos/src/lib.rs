// see: cans/plugins/chaos.md
#![forbid(unsafe_code)]

// TODO (L3): tests/test_chaos.rs per cans/artifacts/config/project-structure.txt.
use std::path::Path;
use std::sync::{Arc, RwLock, RwLockReadGuard};

use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{
    FsInterposer, HandlerCtx, KelirPlugin, MountScope, NetInterposer, PipelineBinder,
};
use kelir_interface::protocol::FilterAction;

// see: cans/plugins/chaos.md#Error-and-return-semantics-(chaos)
pub const RST_INJECTION_ERRNO: &str = "ECONNREFUSED";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FaultDirection {
    #[default]
    Outbound,
    Inbound,
    Both,
}

// see: cans/plugins/chaos.md#Network-chaos
#[derive(Clone, Debug)]
pub struct NetworkFaultRule {
    pub target: String,
    pub direction: FaultDirection,
    pub base_ms: u64,
    pub jitter_ms: u64,
    pub drop_rate: f64,
    pub inject_rst: bool,
    pub bandwidth_kbps: Option<u64>,
}

// see: cans/plugins/chaos.md#Storage-chaos
#[derive(Clone, Debug)]
pub struct StorageFaultRule {
    pub target_path: String,
    pub simulate_error: String,
    pub error_probability: f64,
}

#[derive(Clone, Debug, Default)]
pub struct ChaosConfig {
    pub network: Vec<NetworkFaultRule>,
    pub fs: Vec<StorageFaultRule>,
}

// see: cans/plugins/chaos.md#Service-exports-(chaos)
pub trait FaultInjectionService: Send + Sync {
    fn set_network_rules(&self, rules: Vec<NetworkFaultRule>) -> Result<(), PluginError>;
    fn set_fs_rules(&self, rules: Vec<StorageFaultRule>) -> Result<(), PluginError>;
}

#[derive(Default)]
pub struct FaultTable {
    network: RwLock<Vec<NetworkFaultRule>>,
    fs: RwLock<Vec<StorageFaultRule>>,
}

impl FaultTable {
    fn read_rules<T>(lock: &RwLock<Vec<T>>) -> Result<RwLockReadGuard<'_, Vec<T>>, PluginError> {
        match lock.read() {
            Ok(guard) => Ok(guard),
            Err(_) => Err(PluginError::Halt("chaos fault table poisoned".into())),
        }
    }

    fn write_rules<T>(lock: &RwLock<Vec<T>>, rules: Vec<T>) -> Result<(), PluginError> {
        match lock.write() {
            Ok(mut guard) => {
                *guard = rules;
                Ok(())
            }
            Err(_) => Err(PluginError::Halt("chaos fault table poisoned".into())),
        }
    }
}

// see: cans/plugins/chaos.md#Service-exports-(chaos)
impl FaultInjectionService for FaultTable {
    fn set_network_rules(&self, rules: Vec<NetworkFaultRule>) -> Result<(), PluginError> {
        // TODO (L3): validate dynamic rule mutations against the monotonic lattice before installation.
        Self::write_rules(&self.network, rules)
    }
    fn set_fs_rules(&self, rules: Vec<StorageFaultRule>) -> Result<(), PluginError> {
        // TODO (L3): validate dynamic rule mutations against the monotonic lattice before installation.
        Self::write_rules(&self.fs, rules)
    }
}

#[derive(Clone, Default)]
pub struct ChaosPlugin {
    pub fault_table: Arc<FaultTable>,
}

impl ChaosPlugin {
    pub fn new(config: ChaosConfig) -> Self {
        Self {
            fault_table: Arc::new(FaultTable {
                network: RwLock::new(config.network),
                fs: RwLock::new(config.fs),
            }),
        }
    }

    fn apply_network_rules(
        &self,
        direction: FaultDirection,
        frame: &mut [u8],
    ) -> Result<FilterAction, PluginError> {
        let rules = FaultTable::read_rules(&self.fault_table.network)?;
        for rule in rules
            .iter()
            .filter(|rule| rule.direction == direction || rule.direction == FaultDirection::Both)
        {
            if !self.frame_matches(rule, frame)? {
                continue;
            }
            if rule.inject_rst {
                // see: cans/plugins/chaos.md#Error-and-return-semantics-(chaos)
                return Ok(FilterAction::Drop {
                    errno: Some(RST_INJECTION_ERRNO.into()),
                    reason: format!("chaos: synthetic TCP RST injected for {}", rule.target),
                });
            }
            if self.loss_sampled(rule)? {
                // see: cans/plugins/chaos.md#Error-and-return-semantics-(chaos)
                return Ok(FilterAction::Drop {
                    errno: None,
                    reason: format!("chaos: synthetic packet loss for {}", rule.target),
                });
            }
            // TODO (L3): schedule latency (base_ms plus jitter_ms) and bandwidth_kbps clamping on the matched stream.
        }
        Ok(FilterAction::Forward)
    }

    fn apply_storage_rules(&self, path: &Path) -> Result<FilterAction, PluginError> {
        let rules = FaultTable::read_rules(&self.fault_table.fs)?;
        for rule in rules.iter() {
            if !self.path_matches(&rule.target_path, path)? {
                continue;
            }
            if self.error_sampled(rule)? {
                // see: cans/05-storage-plane.md#Write-gating
                return Ok(FilterAction::Drop {
                    errno: Some(rule.simulate_error.clone()),
                    reason: format!("chaos: simulated storage fault on {}", rule.target_path),
                });
            }
        }
        Ok(FilterAction::Forward)
    }

    fn frame_matches(&self, rule: &NetworkFaultRule, frame: &[u8]) -> Result<bool, PluginError> {
        // TODO (L3): parse L2/L4 headers and match the rule target domain or host pattern.
        let _ = (rule, frame);
        Ok(false)
    }

    fn loss_sampled(&self, rule: &NetworkFaultRule) -> Result<bool, PluginError> {
        // TODO (L3): deterministic synthetic packet loss sampling against drop_rate.
        let _ = rule;
        Ok(false)
    }

    fn path_matches(&self, pattern: &str, path: &Path) -> Result<bool, PluginError> {
        // TODO (L3): glob match of the rule target_path over the guest filesystem path.
        let _ = (pattern, path);
        Ok(false)
    }

    fn error_sampled(&self, rule: &StorageFaultRule) -> Result<bool, PluginError> {
        // TODO (L3): deterministic error sampling against error_probability per write() call.
        let _ = rule;
        Ok(false)
    }
}

impl KelirPlugin for ChaosPlugin {
    // see: cans/plugins/chaos.md#Service-exports-(chaos)
    type Context = Arc<dyn FaultInjectionService>;
    fn context(&self) -> Self::Context {
        self.fault_table.clone()
    }
    fn name(&self) -> &'static str {
        "chaos"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
    // see: cans/plugins/chaos.md#Bidirectional-across-wire-plane
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        pipeline.bind_net_scoped(Arc::new(self.clone()), self.default_scope());
        // see: cans/05-storage-plane.md#Write-gating
        pipeline.bind_fs_scoped(Arc::new(self.clone()), self.default_scope());
        // see: cans/plugins/chaos.md#Service-exports-(chaos)
        pipeline
            .service_registry()
            .register::<dyn FaultInjectionService>(self.fault_table.clone());
    }
}

// see: cans/plugins/chaos.md#Bidirectional-across-wire-plane
impl NetInterposer for ChaosPlugin {
    fn on_net_outbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        self.apply_network_rules(FaultDirection::Outbound, frame)
    }

    fn on_net_inbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        self.apply_network_rules(FaultDirection::Inbound, frame)
    }
}

// see: cans/05-storage-plane.md#Write-gating
impl FsInterposer for ChaosPlugin {
    fn on_fs_write(&self, _data: &mut [u8], ctx: &HandlerCtx) -> Result<FilterAction, PluginError> {
        match &ctx.fs_ctx {
            Some(fs_ctx) => self.apply_storage_rules(&fs_ctx.path),
            None => Ok(FilterAction::Forward),
        }
    }
}
