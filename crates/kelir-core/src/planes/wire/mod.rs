// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#Userspace-network-stack
// see: cans/07-wire-plane.md#Synthetic-DNS-gateway
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex, RwLock};

use kelir_interface::plugin::PluginError;
use kelir_interface::protocol::{FilterAction, StreamKind};

pub const SYNTHETIC_DNS_GATEWAY_IP: Ipv4Addr = Ipv4Addr::new(192, 168, 127, 1);
pub const SYNTHETIC_DNS_PORT: u16 = 53;

pub struct SmoltcpStack {
    interface: Option<smoltcp::iface::Interface>,
}

impl SmoltcpStack {
    pub fn new() -> Self {
        Self { interface: None }
    }

    pub fn poll(&mut self) -> Result<bool, PluginError> {
        // TODO (L3): attach the virtio-net device + SocketSet and run interface.poll over the TCP/UDP sockets.
        let _ = &self.interface;
        Ok(false)
    }
}

impl Default for SmoltcpStack {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DnsRcode {
    NoError,
    NxDomain,
    ServFail,
}

#[derive(Debug, Clone)]
pub struct DnsQuery {
    pub name: String,
    pub qtype: u16,
    pub edns_do: bool,
}

impl DnsQuery {
    pub fn parse(bytes: &[u8]) -> Result<Self, PluginError> {
        // TODO (L3): parse the DNS wire query (name, qtype, EDNS0 OPT record).
        let _ = bytes;
        Err(PluginError::Halt("DNS query parse pending (L3)".into()))
    }

    pub fn downgrade_edns0(&mut self) {
        self.edns_do = false;
    }
}

#[derive(Debug, Clone)]
pub struct DnsResponse {
    pub wire_bytes: Vec<u8>,
    pub rcode: DnsRcode,
    pub edns_downgraded: bool,
}

#[derive(Debug, Clone)]
pub struct L4Stream {
    pub kind: StreamKind,
    pub guest_port: u16,
    pub host_target: String,
}

impl L4Stream {
    pub fn new(guest_port: u16, host_target: String) -> Self {
        Self {
            kind: StreamKind::L4Forward,
            guest_port,
            host_target,
        }
    }
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

pub fn classify_frame(frame: &[u8]) -> FrameClass {
    classify_frame_directional(frame, FrameDirection::Outbound)
}

pub fn classify_frame_directional(frame: &[u8], direction: FrameDirection) -> FrameClass {
    if frame.len() < 14 {
        return FrameClass::TooShort;
    }
    if u16::from_be_bytes([frame[12], frame[13]]) != 0x0800 {
        return FrameClass::NonIp;
    }
    if frame.len() < 15 {
        return FrameClass::TooShort;
    }
    let ihl = usize::from(frame[14] & 0x0f) * 4;
    if ihl < 20 || frame.len() < 14 + ihl + 4 {
        return FrameClass::TooShort;
    }
    let protocol = frame[14 + 9];
    let src_port = u16::from_be_bytes([frame[14 + ihl], frame[14 + ihl + 1]]);
    let dst_port = u16::from_be_bytes([frame[14 + ihl + 2], frame[14 + ihl + 3]]);
    // see: cans/07-wire-plane.md#Synthetic-DNS-gateway
    let dns_port = match direction {
        FrameDirection::Inbound => src_port,
        FrameDirection::Outbound => dst_port,
    };
    match protocol {
        6 => {
            if dns_port == SYNTHETIC_DNS_PORT {
                FrameClass::TcpDns
            } else {
                FrameClass::Tcp
            }
        }
        17 => {
            if dns_port == SYNTHETIC_DNS_PORT {
                FrameClass::UdpDns
            } else {
                FrameClass::UdpOther
            }
        }
        _ => FrameClass::NonTcpUdp,
    }
}

pub struct UdpNatEntry {
    guest_source_port: u16,
    host_socket_fd: i32,
    last_active_unix: u64,
}

pub struct WirePlane {
    synthetic_dns_ip: Ipv4Addr,
    udp_nat_slab: Arc<Mutex<Vec<Option<UdpNatEntry>>>>,
    allowed_domains: Arc<RwLock<Vec<String>>>,
    stack: SmoltcpStack,
}

impl Default for WirePlane {
    fn default() -> Self {
        Self::new()
    }
}

impl WirePlane {
    pub fn new() -> Self {
        Self {
            synthetic_dns_ip: SYNTHETIC_DNS_GATEWAY_IP,
            udp_nat_slab: Arc::new(Mutex::new(Vec::new())),
            allowed_domains: Arc::new(RwLock::new(Vec::new())),
            stack: SmoltcpStack::new(),
        }
    }

    pub fn forward_port(
        &self,
        guest_port: u16,
        host_port: Option<u16>,
    ) -> Result<u16, kelir_interface::plugin::PluginError> {
        // TODO: bind a host socket and record the guest->host mapping; echo requested port until then.
        let _ = (
            &self.udp_nat_slab,
            &self.allowed_domains,
            &self.synthetic_dns_ip,
        );
        Ok(host_port.unwrap_or(guest_port))
    }

    pub fn process_inbound_frame(
        &mut self,
        frame: &mut [u8],
    ) -> Result<kelir_interface::protocol::FilterAction, kelir_interface::plugin::PluginError> {
        match classify_frame_directional(frame, FrameDirection::Inbound) {
            FrameClass::TooShort
            | FrameClass::NonIp
            | FrameClass::NonTcpUdp
            | FrameClass::UdpOther => Ok(FilterAction::Drop {
                errno: None,
                reason: "non-TCP/UDP frame dropped at ingress ring".into(),
            }),
            FrameClass::TcpDns | FrameClass::UdpDns => Ok(FilterAction::Forward),
            FrameClass::Tcp => {
                // TODO (L3): terminate the TCP stream in the embedded smoltcp stack.
                let _ = self.stack.poll()?;
                Ok(FilterAction::Forward)
            }
        }
    }

    pub fn process_outbound_frame(
        &mut self,
        frame: &mut [u8],
    ) -> Result<kelir_interface::protocol::FilterAction, kelir_interface::plugin::PluginError> {
        match classify_frame_directional(frame, FrameDirection::Outbound) {
            FrameClass::TooShort
            | FrameClass::NonIp
            | FrameClass::NonTcpUdp
            | FrameClass::UdpOther => Ok(FilterAction::Drop {
                errno: None,
                reason: "non-TCP/UDP frame dropped at egress ring".into(),
            }),
            FrameClass::TcpDns | FrameClass::UdpDns => {
                // TODO (L3): intercept port 53 here and answer via resolve_synthetic_dns; the resolver is pending.
                let _ = self.resolve_synthetic_dns(frame)?;
                Ok(FilterAction::Forward)
            }
            FrameClass::Tcp => {
                // TODO (L3): run policy + NAT over the outbound TCP stream via the smoltcp stack.
                let _ = self.stack.poll()?;
                Ok(FilterAction::Forward)
            }
        }
    }

    pub fn divert_l4(&self, guest_port: u16, host_target: String) -> Result<L4Stream, PluginError> {
        // TODO (L3): divert the guest TCP stream over stream kind 0x0005 to the host target.
        let _ = L4Stream::new(guest_port, host_target);
        Err(PluginError::Halt(
            "L4 stream diversion over 0x0005 pending (L3)".into(),
        ))
    }

    pub fn resolve_synthetic_dns(
        &self,
        query: &[u8],
    ) -> Result<DnsResponse, kelir_interface::plugin::PluginError> {
        // TODO (L3): parse the query, apply the domain allowlist, answer from host resolver state via the synthetic gateway, EDNS0-downgrade the response.
        let _ = (query, &self.allowed_domains, &self.synthetic_dns_ip);
        Ok(DnsResponse {
            wire_bytes: Vec::new(),
            rcode: DnsRcode::ServFail,
            edns_downgraded: false,
        })
    }

    pub fn reap_stale_udp_sockets(&self) -> Result<usize, kelir_interface::plugin::PluginError> {
        let slab = self.udp_nat_slab.lock().map_err(|_| {
            kelir_interface::plugin::PluginError::Halt("udp nat slab lock poisoned".into())
        })?;
        let reaped = 0;
        for entry in slab.iter().flatten() {
            // TODO: close entries whose last_active_unix is stale; count closed here.
            let _ = (
                entry.guest_source_port,
                entry.host_socket_fd,
                entry.last_active_unix,
            );
        }
        Ok(reaped)
    }
}
