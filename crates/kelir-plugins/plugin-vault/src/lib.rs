// see: cans/plugins/vault.md#Plugin-vault
#![forbid(unsafe_code)]

use kelir_interface::credential_vault::CredentialDescriptor;

#[path = "boot_diversion.flow.rs"]
pub mod boot_diversion;
#[path = "metadata.run.rs"]
pub mod metadata;
#[path = "metadata_endpoint.flow.rs"]
pub mod metadata_endpoint;
#[path = "metadata_endpoint.run.rs"]
pub mod metadata_endpoint_run;
#[path = "service_vault.flow.rs"]
pub mod service_vault;
#[path = "service_vault.run.rs"]
pub mod service_vault_run;
#[path = "synthetic_env.flow.rs"]
pub mod synthetic_env;
#[path = "synthetic_env.run.rs"]
pub mod synthetic_env_run;

pub use boot_diversion::{IMDS_DESTINATION, ImdsBootDiversion};

pub use metadata_endpoint::{CredentialBrokerCtx, MetadataEndpoint, VaultPlugin};
pub use service_vault::{
    CarrierInjector, MtlsMaterial, VAULT_PLACEHOLDER_PREFIX, load_mtls_material,
};
pub use synthetic_env::{SyntheticEnvFile, VaultSyntheticFiles};

// see: cans/plugins/vault.md#Metadata-service-(vault)
pub const METADATA_CREDENTIALS_PREFIX: &str = "/kelir/credentials/";

// see: cans/07-wire-plane.md#Pre-policy-drop-zones
// L3-GAP: [governance_vault]: pin this constant against the kelir-engine vsock transport host-bridge identity; slop: CID constant unpinned, bridge identity drifts, wrong peer served
pub const WIRE_PLANE_BRIDGE_CID: u32 = 2;

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.vault.metadata_service
#[derive(Clone, Debug)]
pub struct VaultConfig {
    pub metadata_vsock_port: u32,
    pub token_ttl_seconds: u32,
    // see: cans/plugins/vault.md#Credential-descriptors-(vault)
    pub credentials: Vec<CredentialDescriptor>,
}
