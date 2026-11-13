// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#Userspace-network-stack
use std::net::Ipv4Addr;

use crate::frame_layout_views::{
    ETH_HEADER_BYTES, ETHERTYPE_IPV4, EthView, IP_PROTO_TCP, IPV4_MIN_HEADER_BYTES, Ipv4View,
    TCP_MIN_HEADER_BYTES, TcpView,
};
use crate::plugin_model::PluginError;
use crate::protocol_wire::FilterAction;
use crate::vsock_mesh::VAULT_METADATA_VSOCK_PORT;

pub const SYNTHETIC_DNS_GATEWAY_IP: Ipv4Addr = Ipv4Addr::new(192, 168, 127, 1);
pub const SYNTHETIC_DNS_PORT: u16 = 53;

// see: cans/07-wire-plane.md#Pre-policy-drop-zones
pub const LINK_LOCAL_IMDS_IP: Ipv4Addr = Ipv4Addr::new(169, 254, 169, 254);

const UDP_HEADER_BYTES: usize = 8;
const L4_PORT_PAIR_BYTES: usize = 4;
const IP_PROTO_UDP: u8 = 17;

mod udp {
    pub(super) const SRC_PORT: usize = 0;
    pub(super) const DST_PORT: usize = 2;
}

struct UdpView<'a> {
    bytes: &'a [u8],
}

impl<'a> UdpView<'a> {
    fn new_unchecked(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    fn src_port(&self) -> u16 {
        u16::from_be_bytes([self.bytes[udp::SRC_PORT], self.bytes[udp::SRC_PORT + 1]])
    }

    fn dst_port(&self) -> u16 {
        u16::from_be_bytes([self.bytes[udp::DST_PORT], self.bytes[udp::DST_PORT + 1]])
    }
}

pub trait NetOps: Send + Sync {
    fn intercept_ethernet_frame(
        &self,
        direction: FrameDirection,
        frame: &mut [u8],
    ) -> Result<FilterAction, PluginError>;
    fn apply_wiring(&self, wiring: &WireWiring);
}

// see: cans/07-wire-plane.md#DNS-policy-filter
#[derive(Debug, Clone, Default)]
pub struct WireWiring {
    pub sinkhole_allowlist: Vec<String>,
    pub sinkhole_drop_all: bool,
    pub ssh_agent_bridge: bool,
    pub vault_placeholders: Vec<String>,
    pub l4_proxies: Vec<(u16, String)>,
    pub allow_udp_proxied: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameClass {
    TooShort,
    NonIp,
    NonTcpUdp,
    Tcp,
    TcpDns,
    UdpDns,
    UdpOther,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameDirection {
    Inbound,
    Outbound,
}

// see: cans/07-wire-plane.md#Pre-policy-drop-zones
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkLocalRoute {
    Drop,
    VaultImds { vsock_port: u32 },
}

// see: cans/07-wire-plane.md#Pre-policy-drop-zones
// TODO (L3) [wire_plane]: drop non-link-local cloud IMDS endpoints (e.g. Azure 168.63.129.16) at ingress; only the link-local 169.254.169.254 exception reaches the vault; slop: cloud IMDS endpoint reaches vault, quarantine bypassed
pub fn link_local_route(frame: &[u8], direction: FrameDirection) -> Option<LinkLocalRoute> {
    let remote = frame_remote_ip(frame, direction)?;
    if !remote.is_link_local() {
        return None;
    }
    if direction == FrameDirection::Outbound && remote == LINK_LOCAL_IMDS_IP {
        return Some(LinkLocalRoute::VaultImds {
            vsock_port: VAULT_METADATA_VSOCK_PORT,
        });
    }
    Some(LinkLocalRoute::Drop)
}

// see: cans/07-wire-plane.md#Pre-policy-drop-zones
fn frame_remote_ip(frame: &[u8], direction: FrameDirection) -> Option<Ipv4Addr> {
    if frame.len() < ETH_HEADER_BYTES {
        return None;
    }
    if EthView::new_unchecked(frame).etype() != ETHERTYPE_IPV4 {
        return None;
    }
    let ip = Ipv4View::new_unchecked(&frame[ETH_HEADER_BYTES..]);
    let octets = match direction {
        FrameDirection::Inbound => ip.src_checked(),
        FrameDirection::Outbound => ip.dst_checked(),
    }?;
    Some(Ipv4Addr::from(octets))
}

pub fn classify_frame(frame: &[u8]) -> FrameClass {
    classify_frame_directional(frame, FrameDirection::Outbound)
}

pub fn classify_frame_directional(frame: &[u8], direction: FrameDirection) -> FrameClass {
    if frame.len() < ETH_HEADER_BYTES {
        return FrameClass::TooShort;
    }
    if EthView::new_unchecked(frame).etype() != ETHERTYPE_IPV4 {
        return FrameClass::NonIp;
    }
    let ip = Ipv4View::new_unchecked(&frame[ETH_HEADER_BYTES..]);
    let Some(ihl) = ip.ihl_bytes_checked() else {
        return FrameClass::TooShort;
    };
    if ihl < IPV4_MIN_HEADER_BYTES || frame.len() < ETH_HEADER_BYTES + ihl + L4_PORT_PAIR_BYTES {
        return FrameClass::TooShort;
    }
    match ip.proto() {
        IP_PROTO_TCP => {
            if is_synthetic_dns(frame, direction) {
                FrameClass::TcpDns
            } else {
                FrameClass::Tcp
            }
        }
        IP_PROTO_UDP => {
            if is_synthetic_dns(frame, direction) {
                FrameClass::UdpDns
            } else {
                FrameClass::UdpOther
            }
        }
        _ => FrameClass::NonTcpUdp,
    }
}

// see: cans/07-wire-plane.md#Synthetic-DNS-gateway
pub fn is_synthetic_dns(frame: &[u8], direction: FrameDirection) -> bool {
    if frame.len() < ETH_HEADER_BYTES {
        return false;
    }
    if EthView::new_unchecked(frame).etype() != ETHERTYPE_IPV4 {
        return false;
    }
    let ip = Ipv4View::new_unchecked(&frame[ETH_HEADER_BYTES..]);
    let Some(ihl) = ip.ihl_bytes_checked() else {
        return false;
    };
    if ihl < IPV4_MIN_HEADER_BYTES || frame.len() < ETH_HEADER_BYTES + ihl + L4_PORT_PAIR_BYTES {
        return false;
    }
    let gateway_octets = SYNTHETIC_DNS_GATEWAY_IP.octets();
    let gateway_ip = match direction {
        FrameDirection::Inbound => ip.src(),
        FrameDirection::Outbound => ip.dst(),
    };
    if gateway_ip != gateway_octets {
        return false;
    }
    let protocol = ip.proto();
    if protocol != IP_PROTO_TCP && protocol != IP_PROTO_UDP {
        return false;
    }
    let l4_start = ETH_HEADER_BYTES + ihl;
    let port = match protocol {
        IP_PROTO_TCP => {
            let tcp = TcpView::new_unchecked(&frame[l4_start..]);
            match direction {
                FrameDirection::Inbound => tcp.src_port(),
                FrameDirection::Outbound => tcp.dst_port(),
            }
        }
        _ => {
            let udp = UdpView::new_unchecked(&frame[l4_start..]);
            match direction {
                FrameDirection::Inbound => udp.src_port(),
                FrameDirection::Outbound => udp.dst_port(),
            }
        }
    };
    port == SYNTHETIC_DNS_PORT
}

// see: cans/07-wire-plane.md#Synthetic-DNS-gateway
pub fn dns_payload(frame: &[u8], direction: FrameDirection) -> Option<&[u8]> {
    if !is_synthetic_dns(frame, direction) {
        return None;
    }
    let ip = Ipv4View::new_unchecked(&frame[ETH_HEADER_BYTES..]);
    let l4_start = ETH_HEADER_BYTES + ip.ihl_bytes_checked()?;
    match ip.proto() {
        IP_PROTO_UDP => {
            if frame.len() < l4_start + UDP_HEADER_BYTES {
                return None;
            }
            Some(&frame[l4_start + UDP_HEADER_BYTES..])
        }
        IP_PROTO_TCP => {
            let tcp = TcpView::new_unchecked(&frame[l4_start..]);
            let header_bytes = tcp.header_bytes_checked()?;
            if header_bytes < TCP_MIN_HEADER_BYTES || frame.len() < l4_start + header_bytes {
                return None;
            }
            Some(&frame[l4_start + header_bytes..])
        }
        _ => None,
    }
}
