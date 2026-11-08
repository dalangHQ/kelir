// see: cans/plugins/vault.md
// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.vault.metadata_service
use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;

pub use kelir_interface::credential::{CredentialDescriptor, CredentialKind};

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.vault.metadata_service.token_ttl_seconds
pub const DEFAULT_TOKEN_TTL_SECONDS: u32 = 60;
// see: cans/plugins/vault.md
pub const MAX_TOKEN_TTL_SECONDS: u32 = 900;

// see: cans/plugins/vault.md
#[derive(Debug, Clone)]
pub struct TokenRequest {
    pub name: String,
    pub ttl_seconds: u32,
}

// see: cans/plugins/vault.md
#[derive(Debug, Clone)]
pub struct IssuedToken {
    pub token: String,
    pub ttl_seconds: u32,
    pub scope: String,
    pub audience: String,
}

// see: cans/plugins/vault.md
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultError {
    Enodata(String),
    Eperm(String),
    Eacces(String),
    Einval(String),
}

impl VaultError {
    pub fn errno(&self) -> &'static str {
        match self {
            Self::Enodata(_) => "ENODATA",
            Self::Eperm(_) => "EPERM",
            Self::Eacces(_) => "EACCES",
            Self::Einval(_) => "EINVAL",
        }
    }

    pub fn reason(&self) -> &str {
        match self {
            Self::Enodata(reason)
            | Self::Eperm(reason)
            | Self::Eacces(reason)
            | Self::Einval(reason) => reason,
        }
    }
}

// see: cans/plugins/vault.md
#[async_trait]
pub trait CredentialBrokerService: Send + Sync {
    async fn issue(&self, request: &TokenRequest) -> Result<IssuedToken, VaultError>;
}

// see: cans/plugins/vault.md
pub struct CredentialBroker {
    descriptors: Vec<CredentialDescriptor>,
    outstanding: Mutex<HashMap<String, IssuedToken>>,
}

impl CredentialBroker {
    pub fn new(descriptors: Vec<CredentialDescriptor>) -> Self {
        Self {
            descriptors,
            outstanding: Mutex::new(HashMap::new()),
        }
    }

    pub fn revoke_session_tokens(&self) {
        if let Ok(mut outstanding) = self.outstanding.lock() {
            outstanding.clear();
        }
    }
}

#[async_trait]
impl CredentialBrokerService for CredentialBroker {
    async fn issue(&self, request: &TokenRequest) -> Result<IssuedToken, VaultError> {
        let Some(descriptor) = self.descriptors.iter().find(|d| d.name == request.name) else {
            return Err(VaultError::Enodata(format!(
                "credential {} is not registered",
                request.name
            )));
        };
        if request.ttl_seconds > MAX_TOKEN_TTL_SECONDS {
            return Err(VaultError::Einval(format!(
                "ttl {} exceeds the {MAX_TOKEN_TTL_SECONDS}s ceiling",
                request.ttl_seconds
            )));
        }
        // TODO (L3): MountScope confinement (EPERM), audience locking (EACCES), and keyring minting with zeroize-on-drop per cans/plugins/vault.md.
        let token = IssuedToken {
            token: String::new(),
            ttl_seconds: request.ttl_seconds,
            scope: String::new(),
            audience: descriptor.audience.clone(),
        };
        let mut outstanding = self
            .outstanding
            .lock()
            .map_err(|_| VaultError::Eperm("credential broker poisoned".into()))?;
        outstanding.insert(request.name.clone(), token.clone());
        Ok(token)
    }
}
