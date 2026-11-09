use std::path::PathBuf;

use kelir_interface::credential_vault::CredentialDescriptor;

#[derive(Debug, Clone)]
pub struct SinkholeConfig {
    pub allowlist: Vec<String>,
    pub drop_all: bool,
}

#[derive(Debug, Clone)]
pub struct Mtls {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct L4Proxy {
    pub listen_port: u16,
    pub destination: String,
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
}
