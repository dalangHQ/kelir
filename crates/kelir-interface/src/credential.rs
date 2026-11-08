// see: cans/plugins/vault.md
use serde::{Deserialize, Serialize};

// see: cans/plugins/vault.md
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialKind {
    BearerToken,
    SigV4,
    MtlsBundle,
}

// see: cans/plugins/vault.md
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialDescriptor {
    pub name: String,
    pub kind: CredentialKind,
    pub audience: String,
    pub placeholder: String,
}
