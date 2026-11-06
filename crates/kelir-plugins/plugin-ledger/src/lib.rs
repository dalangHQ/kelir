// see: cans/plugins/ledger.md
#![forbid(unsafe_code)]

use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{KelirPlugin, LifecycleParticipant, MountScope, TrapInterposer};
use kelir_interface::telemetry::CausalDelta;

pub trait LedgerSink: Send + Sync {
    fn emit_delta(&self, delta: &CausalDelta) -> Result<(), PluginError>;
}

#[derive(Clone)]
pub struct LedgerPlugin {
    pub merkle_enabled: bool,
}

impl LedgerPlugin {
    pub fn new(merkle_enabled: bool) -> Self {
        Self { merkle_enabled }
    }
}

impl KelirPlugin for LedgerPlugin {
    fn name(&self) -> &'static str {
        "ledger"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
}

impl LifecycleParticipant for LedgerPlugin {}
impl TrapInterposer for LedgerPlugin {}
