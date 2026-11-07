// see: cans/plugins/vault.md
// see: cans/07-wire-plane.md
#![forbid(unsafe_code)]

use std::sync::Arc;

use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{
    KelirPlugin, LifecycleParticipant, MountScope, PipelineBinder, VirtioDevice,
};

pub const METADATA_HOST: &str = "169.254.169.254";
pub const TOKEN_TTL_DEFAULT_SECS: u64 = 60;
pub const TOKEN_TTL_MAX_SECS: u64 = 900;

#[derive(Debug, Clone)]
pub struct CredentialToken {
    pub token: String,
    pub ttl_seconds: u64,
    pub scope: String,
    pub audience: String,
}

#[derive(Debug, Clone)]
pub struct CredentialBrokerService {
    pub token_ttl_secs: u64,
}

impl CredentialBrokerService {
    pub fn new() -> Self {
        Self {
            token_ttl_secs: TOKEN_TTL_DEFAULT_SECS,
        }
    }

    pub fn metadata_path(name: &str) -> String {
        format!("http://{METADATA_HOST}/kelir/credentials/{name}")
    }

    pub fn issue(
        &self,
        name: &str,
        scope: &str,
        audience: &str,
        ttl_seconds: u64,
    ) -> Result<CredentialToken, PluginError> {
        if name.is_empty() {
            return Err(PluginError::Halt(
                "vault: unregistered credential name (ENODATA)".into(),
            ));
        }
        if ttl_seconds > TOKEN_TTL_MAX_SECS {
            return Err(PluginError::Halt(
                "vault: TTL request above 900s ceiling (EINVAL)".into(),
            ));
        }
        let _ = (scope, audience);
        // TODO (L3): mint the signed opaque bearer from the host keyring over the vsock metadata service; no raw secret enters guest RAM.
        Err(PluginError::Halt(
            "vault: credential issuance needs the host keyring broker (L3)".into(),
        ))
    }
}

impl Default for CredentialBrokerService {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct VaultPlugin {
    pub broker: CredentialBrokerService,
}

impl VaultPlugin {
    pub fn new() -> Self {
        Self {
            broker: CredentialBrokerService::new(),
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
        MountScope::default().with_devices([VirtioDevice::Vsock])
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        pipeline
            .service_registry()
            .register(Arc::new(self.broker.clone()));
    }
}

impl LifecycleParticipant for VaultPlugin {}
