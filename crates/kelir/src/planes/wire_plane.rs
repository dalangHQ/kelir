use std::path::PathBuf;

use kelir_interface::credential_vault::CredentialDescriptor;

#[derive(Debug, Clone)]
pub struct SinkholeConfig {
    pub allowlist: Vec<String>,
    pub drop_all: bool,
}

// see: cans/plugins/vault.md#In-ring-carrier-secret-injection
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mtls {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct L4Proxy {
    pub listen_port: u16,
    pub destination: String,
    // see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
    pub forward_unix_socket: Option<PathBuf>,
    pub inject_mtls: Option<Mtls>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapFilter {
    AllowUdpProxied,
}

#[derive(Debug, Clone)]
pub struct WireConfig {
    pub sinkhole: SinkholeConfig,
    pub ssh_agent_bridge: bool,
    pub vaults: Vec<CredentialDescriptor>,
    pub l4_proxies: Vec<L4Proxy>,
    pub tap_filter: TapFilter,
    // see: cans/artifacts/examples/fixtures/kelir.yaml#wire.userspace_netstack
    pub userspace_netstack: bool,
    // see: cans/artifacts/examples/fixtures/kelir.yaml#wire.default_egress
    pub default_egress: String,
}

impl WireConfig {
    // see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
    pub fn divert_port_to_unix(mut self, listen_port: u16, socket: impl Into<PathBuf>) -> Self {
        self.l4_proxies.push(L4Proxy {
            listen_port,
            destination: String::new(),
            forward_unix_socket: Some(socket.into()),
            inject_mtls: None,
        });
        self
    }
}
