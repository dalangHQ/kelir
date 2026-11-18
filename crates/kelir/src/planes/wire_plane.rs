use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// see: cans/07-wire-plane.md#Sinkhole-rules
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SinkholeRule {
    pub host: String,
    pub allow: bool,
}

// see: cans/07-wire-plane.md#Sinkhole-rules
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SinkholeConfig {
    pub rules: Vec<SinkholeRule>,
}

// see: cans/plugins/vault.md#In-ring-carrier-secret-injection
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MtlsConfig {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

// see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct L4Proxy {
    pub listen_port: u16,
    pub destination: String,
    pub forward_unix_socket: Option<PathBuf>,
    // TODO (L3) [wp_12_l4_diversion_passthrough_tls]: raw packet splicing and streaming TLS termination at the diverted upstream dial.; slop: splicing stubbed, TLS terminated with custom CA or plaintext inspected
    pub inject_mtls: Option<MtlsConfig>,
}

// see: cans/07-wire-plane.md#Wire-plane
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TapFilter {
    #[default]
    AllowUdpProxied,
}

// see: cans/07-wire-plane.md#Wire-plane
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireConfig {
    pub sinkhole: SinkholeConfig,
    pub ssh_agent_bridge: bool,
    pub dial_timeout_ms: u64,
    pub l4_proxies: Vec<L4Proxy>,
    pub tap_filter: TapFilter,
    // see: cans/artifacts/examples/fixtures/kelir.yaml#wire.userspace_netstack
    pub userspace_netstack: bool,
    // see: cans/artifacts/examples/fixtures/kelir.yaml#wire.default_egress
    pub default_egress: String,
}

// see: cans/07-wire-plane.md#Upstream-dial-transport
impl Default for WireConfig {
    fn default() -> Self {
        Self {
            sinkhole: SinkholeConfig::default(),
            ssh_agent_bridge: false,
            dial_timeout_ms: 5000,
            l4_proxies: Vec::new(),
            tap_filter: TapFilter::default(),
            userspace_netstack: false,
            default_egress: String::new(),
        }
    }
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
