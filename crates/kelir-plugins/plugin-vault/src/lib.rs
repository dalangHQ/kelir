// see: cans/plugins/vault.md#Plugin-vault
#![forbid(unsafe_code)]

#[path = "metadata.run.rs"]
pub mod metadata;
#[path = "metadata_endpoint.flow.rs"]
pub mod metadata_endpoint;
#[path = "service_vault.flow.rs"]
pub mod service_vault;

pub use metadata_endpoint::{CredentialBrokerCtx, MetadataEndpoint, VaultPlugin};

// see: cans/plugins/vault.md#Metadata-service-(vault)
pub const METADATA_CREDENTIALS_PREFIX: &str = "/kelir/credentials/";

// see: cans/07-wire-plane.md#Pre-policy-drop-zones
// TODO (L3) [governance_vault]: pin this constant against the kelir-engine vsock transport host-bridge identity; slop: CID constant unpinned, bridge identity drifts, wrong peer served
pub const WIRE_PLANE_BRIDGE_CID: u32 = 2;

// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.vault.metadata_service
#[derive(Clone, Debug)]
pub struct VaultConfig {
    pub metadata_vsock_port: u32,
    pub token_ttl_seconds: u32,
}
