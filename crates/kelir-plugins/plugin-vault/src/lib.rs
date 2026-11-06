// see: cans/plugins/vault.md
#![forbid(unsafe_code)]

use kelir_interface::errors::PluginError;
use kelir_interface::plugin::{KelirPlugin, LifecycleParticipant, MountScope, NetInterposer};
use kelir_interface::protocol::FilterAction;

#[derive(Clone)]
pub struct CredentialBrokerService;

#[derive(Clone)]
pub struct VaultPlugin {
    pub broker: CredentialBrokerService,
}

impl VaultPlugin {
    pub fn new() -> Self {
        Self {
            broker: CredentialBrokerService,
        }
    }
}

impl Default for VaultPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for VaultPlugin {
    type Context = ();
    fn context(&self) {}
    fn name(&self) -> &'static str {
        "vault"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
}

impl LifecycleParticipant for VaultPlugin {}
impl NetInterposer for VaultPlugin {
    fn on_net_outbound(&self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        // TODO: Intercept 169.254.169.254 requests and broker short-lived tokens
        Ok(FilterAction::Forward)
    }
}
