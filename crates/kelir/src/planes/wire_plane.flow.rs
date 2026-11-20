// see: cans/07-wire-plane.md#Wire-plane
use std::path::PathBuf;

use super::wire_plane::{L4Proxy, SinkholeConfig, TapFilter, WireConfig};

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
