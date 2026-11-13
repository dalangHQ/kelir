// see: cans/plugins/chaos.md#Plugin-chaos
#![forbid(unsafe_code)]

// TODO (L3) [plugin_engine]: tests/test_chaos.rs per cans/artifacts/config/project-structure.txt; slop: tests/test_chaos.rs missing; chaos faults untested, drills pass vacuously
use kelir_interface::plugin_model::PluginError;
use kelir_interface::protocol_wire::Errno;

pub use kelir_interface::chaos_types::FaultDirection;

#[path = "fault_table.flow.rs"]
pub mod fault_table;

pub use fault_table::{ChaosPlugin, FaultTable};

// see: cans/plugins/chaos.md#Error-and-return-semantics-(chaos)
pub const RST_INJECTION_ERRNO: Errno = Errno::Econnrefused;

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
    pub simulate_error: Errno,
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
