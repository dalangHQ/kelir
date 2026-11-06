// see: cans/plugins/db-proxy.md
#![forbid(unsafe_code)]

// TODO (L3): tests/test_db_proxy.rs per cans/artifacts/config/project-structure.txt.
use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{KelirPlugin, MountScope, NetInterposer};
use kelir_interface::protocol::FilterAction;

#[derive(Clone)]
pub struct SqlProxyService;

#[derive(Clone)]
pub struct DbProxyPlugin {
    pub service: SqlProxyService,
}

impl DbProxyPlugin {
    pub fn new() -> Self {
        Self {
            service: SqlProxyService,
        }
    }
}

impl Default for DbProxyPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for DbProxyPlugin {
    fn name(&self) -> &'static str {
        "db-proxy"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
}

impl NetInterposer for DbProxyPlugin {
    fn on_net_outbound(&self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        // TODO: Intercept pgwire (port 5432) and forward over DB_SQLITE (0x0011)
        Ok(FilterAction::Forward)
    }
}
