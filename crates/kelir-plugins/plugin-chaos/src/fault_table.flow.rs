// see: cans/plugins/chaos.md#Service-exports-(chaos)
use std::sync::{Arc, RwLock, RwLockReadGuard};

use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{KelirPlugin, PipelineBinder, PluginDeps};

use crate::{ChaosConfig, FaultInjectionService, NetworkFaultRule, StorageFaultRule};

// see: cans/plugins/chaos.md#Service-exports-(chaos)
#[derive(Default)]
pub struct FaultTable {
    network: RwLock<Vec<NetworkFaultRule>>,
    storage: RwLock<Vec<StorageFaultRule>>,
}

// TODO (L3) [plugin_engine]: remove this allow when the stubbed L3 callers land and read this item again.; slop: the allow hides a dead item forever, the stubbed callers never land, fault-table wiring stays unwired while the gate reports clean.
#[allow(dead_code)]
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
        // TODO (L3) [plugin_engine]: validate dynamic rule mutations against the monotonic lattice before installation; slop: lattice check skipped, non-monotonic rule installed, later rules silently dropped
        Self::write_rules(&self.network, rules)
    }
    fn set_storage_rules(&self, rules: Vec<StorageFaultRule>) -> Result<(), PluginError> {
        // TODO (L3) [plugin_engine]: validate dynamic rule mutations against the monotonic lattice before installation; slop: lattice check skipped, non-monotonic rule installed, later rules silently dropped
        Self::write_rules(&self.storage, rules)
    }
}

#[derive(Clone, Default, PluginDeps)]
pub struct ChaosPlugin {
    pub fault_table: Arc<FaultTable>,
}

impl ChaosPlugin {
    pub fn new(config: ChaosConfig) -> Self {
        Self {
            fault_table: Arc::new(FaultTable {
                network: RwLock::new(config.network),
                storage: RwLock::new(config.storage),
            }),
        }
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
    // see: cans/plugins/chaos.md#Bidirectional-across-wire-plane
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        pipeline.bind_net_scoped(Arc::new(self.clone()), self.default_scope());
        // see: cans/05-storage-plane.md#Write-gating
        pipeline.bind_fs_scoped(Arc::new(self.clone()), self.default_scope());
    }
}

// see: cans/plugins/chaos.md#Plugin-chaos

// TODO (L3) [plugin_engine]: construct ChaosPlugin via KelirBuilder::named in the kelir facade crate; a dependency on that crate here is a cargo cycle since the facade depends on plugin-chaos per cans/02-plugin-model.md#KelirPlugin-trait; slop: facade cycle: plugin-chaos depends on facade, build breaks cyclically
