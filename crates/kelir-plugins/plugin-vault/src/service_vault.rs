// see: cans/plugins/vault.md#Plugin-vault
// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.vault.metadata_service
use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;

pub use kelir_interface::credential_vault::{CredentialDescriptor, CredentialKind};

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.vault.metadata_service.token_ttl_seconds
pub const DEFAULT_TOKEN_TTL_SECONDS: u32 = 60;
// see: cans/plugins/vault.md#Out-of-band-credential-brokerage
pub const MAX_TOKEN_TTL_SECONDS: u32 = 900;

// see: cans/plugins/vault.md#Out-of-band-credential-brokerage
#[derive(Debug, Clone)]
pub struct TokenRequest {
    pub name: String,
    pub ttl_seconds: u32,
}

// see: cans/plugins/vault.md#Memory-isolation-and-zeroization
#[derive(Clone)]
pub struct IssuedToken {
    pub token: String,
    pub ttl_seconds: u32,
    pub scope: String,
    pub audience: String,
}

// see: cans/plugins/vault.md#Memory-isolation-and-zeroization
impl std::fmt::Debug for IssuedToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IssuedToken")
            .field("token", &"<redacted>")
            .field("ttl_seconds", &self.ttl_seconds)
            .field("scope", &self.scope)
            .field("audience", &self.audience)
            .finish()
    }
}

// see: cans/plugins/vault.md#Memory-isolation-and-zeroization
impl Drop for IssuedToken {
    fn drop(&mut self) {
        zeroize_token(&mut self.token);
    }
}

// see: cans/plugins/vault.md#Memory-isolation-and-zeroization
fn zeroize_token(token: &mut String) {
    let len = token.len();
    token.clear();
    token.push_str(&"\0".repeat(len));
    token.clear();
}

// see: cans/plugins/vault.md#Revocation
#[derive(Debug, Clone)]
struct OutstandingToken {
    session_id: String,
    expires_at_unix: u64,
    token_digest: [u8; 32],
}

// see: cans/plugins/vault.md#Return-errno-and-rejection-behavior-(vault)
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

// see: cans/plugins/vault.md#Service-exports-(vault)
#[async_trait]
pub trait CredentialBrokerService: Send + Sync {
    async fn issue(&self, request: &TokenRequest) -> Result<IssuedToken, VaultError>;
    // see: cans/plugins/vault.md#Revocation
    async fn revoke_all(&self, session_id: &str) -> Result<usize, VaultError>;
    // see: cans/plugins/vault.md#Revocation
    async fn validate_token(&self, session_id: &str, token: &str) -> Result<bool, VaultError>;
}

// see: cans/plugins/vault.md#Service-exports-(vault)
// TODO (L3): pin host keyring memory with mlock(2) through the driver/transport enclave; plugins forbid unsafe.
pub struct CredentialBroker {
    descriptors: Vec<CredentialDescriptor>,
    session_id: Mutex<Option<String>>,
    outstanding: Mutex<HashMap<String, OutstandingToken>>,
    issuance_nonce: AtomicU64,
}

impl CredentialBroker {
    pub fn new(descriptors: Vec<CredentialDescriptor>) -> Self {
        Self {
            descriptors,
            session_id: Mutex::new(None),
            outstanding: Mutex::new(HashMap::new()),
            issuance_nonce: AtomicU64::new(0),
        }
    }

    fn lock_session(&self) -> Result<MutexGuard<'_, Option<String>>, VaultError> {
        self.session_id
            .lock()
            .map_err(|_| VaultError::Eperm("credential broker session scope poisoned".into()))
    }

    fn lock_outstanding(
        &self,
    ) -> Result<MutexGuard<'_, HashMap<String, OutstandingToken>>, VaultError> {
        self.outstanding
            .lock()
            .map_err(|_| VaultError::Eperm("credential broker poisoned".into()))
    }

    // see: cans/plugins/vault.md#Revocation
    pub fn bind_session(&self, session_id: &str) -> Result<(), VaultError> {
        let mut session = self.lock_session()?;
        match session.as_deref() {
            Some(bound) if bound != session_id => Err(VaultError::Eperm(
                "credential broker session scope is immutable".into(),
            )),
            _ => {
                *session = Some(session_id.to_string());
                Ok(())
            }
        }
    }

    // see: cans/plugins/vault.md#Memory-isolation-and-zeroization
    pub fn revoke_expired(&self, now_unix: u64) -> Result<usize, VaultError> {
        let mut outstanding = self.lock_outstanding()?;
        let before = outstanding.len();
        outstanding.retain(|_, entry| entry.expires_at_unix > now_unix);
        Ok(before - outstanding.len())
    }

    // see: cans/plugins/vault.md#Revocation
    fn revoke_session(&self, session_id: &str) -> Result<usize, VaultError> {
        let mut outstanding = self.lock_outstanding()?;
        let before = outstanding.len();
        outstanding.retain(|_, entry| entry.session_id != session_id);
        Ok(before - outstanding.len())
    }

    // see: cans/plugins/vault.md#Revocation
    fn validate_session_token(&self, session_id: &str, token: &str) -> Result<bool, VaultError> {
        let now_unix = unix_now()?;
        let digest = *blake3::hash(token.as_bytes()).as_bytes();
        let outstanding = self.lock_outstanding()?;
        Ok(outstanding.values().any(|entry| {
            entry.session_id == session_id
                && entry.token_digest == digest
                && entry.expires_at_unix > now_unix
        }))
    }

    // see: cans/plugins/vault.md#Memory-isolation-and-zeroization
    fn bound_session(&self) -> Result<String, VaultError> {
        self.lock_session()?
            .clone()
            .ok_or_else(|| VaultError::Eperm("credential broker has no session scope bound".into()))
    }
}

#[async_trait]
impl CredentialBrokerService for CredentialBroker {
    // see: cans/plugins/vault.md#Out-of-band-credential-brokerage
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
        let session_id = self.bound_session()?;
        let now_unix = unix_now()?;
        self.revoke_expired(now_unix)?;
        let expires_at_unix = now_unix
            .checked_add(u64::from(request.ttl_seconds))
            .ok_or_else(|| VaultError::Einval("token expiry overflow".into()))?;
        let nonce = self.issuance_nonce.fetch_add(1, Ordering::AcqRel);
        // TODO (L3): MountScope confinement (EPERM) and audience locking (EACCES); scope derives from the builder MountScope per cans/plugins/vault.md.
        let token = IssuedToken {
            token: mint_token(nonce),
            ttl_seconds: request.ttl_seconds,
            scope: descriptor.placeholder.clone(),
            audience: descriptor.audience.clone(),
        };
        let token_digest = *blake3::hash(token.token.as_bytes()).as_bytes();
        let mut outstanding = self.lock_outstanding()?;
        outstanding.insert(
            request.name.clone(),
            OutstandingToken {
                session_id,
                expires_at_unix,
                token_digest,
            },
        );
        Ok(token)
    }

    // see: cans/plugins/vault.md#Revocation
    async fn revoke_all(&self, session_id: &str) -> Result<usize, VaultError> {
        self.revoke_session(session_id)
    }

    // see: cans/plugins/vault.md#Revocation
    async fn validate_token(&self, session_id: &str, token: &str) -> Result<bool, VaultError> {
        self.validate_session_token(session_id, token)
    }
}

// see: cans/plugins/vault.md#Memory-isolation-and-zeroization
fn unix_now() -> Result<u64, VaultError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs())
        .map_err(|_| VaultError::Eperm("host clock is behind the unix epoch".into()))
}

// see: cans/plugins/vault.md#Out-of-band-credential-brokerage
// TODO (L3): swap the std hashmap entropy draw for the host keyring CSPRNG mint per cans/plugins/vault.md#Memory-isolation-and-zeroization.
fn mint_token(nonce: u64) -> String {
    let mut material = [0u8; 24];
    material[..8].copy_from_slice(&entropy_draw().to_le_bytes());
    material[8..16].copy_from_slice(&nonce.to_le_bytes());
    material[16..].copy_from_slice(&entropy_draw().to_le_bytes());
    hex_encode(&material)
}

// see: cans/plugins/vault.md#Out-of-band-credential-brokerage
fn entropy_draw() -> u64 {
    RandomState::new().build_hasher().finish()
}

// see: cans/plugins/vault.md#Out-of-band-credential-brokerage
fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[usize::from(byte >> 4)] as char);
        encoded.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    encoded
}
