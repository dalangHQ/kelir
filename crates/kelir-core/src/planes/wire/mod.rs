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
    match protocol {
        6 => {
            if is_synthetic_dns(frame, direction) {
                FrameClass::TcpDns
            } else {
                FrameClass::Tcp
            }
        }
        17 => {
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
    if frame.len() < 14 {
        return false;
    }
    if u16::from_be_bytes([frame[12], frame[13]]) != 0x0800 {
        return false;
    }
    let ihl = usize::from(frame[14] & 0x0f) * 4;
    if ihl < 20 || frame.len() < 14 + ihl + 4 {
        return false;
    }
    let gateway_octets = SYNTHETIC_DNS_GATEWAY_IP.octets();
    let gateway_ip = match direction {
        FrameDirection::Inbound => &frame[14 + 12..14 + 16],
        FrameDirection::Outbound => &frame[14 + 16..14 + 20],
    };
    if gateway_ip != gateway_octets {
        return false;
    }
    let protocol = frame[14 + 9];
    if protocol != 6 && protocol != 17 {
        return false;
    }
    let port_offset = match direction {
        FrameDirection::Inbound => 14 + ihl,
        FrameDirection::Outbound => 14 + ihl + 2,
    };
    let port = u16::from_be_bytes([frame[port_offset], frame[port_offset + 1]]);
    port == SYNTHETIC_DNS_PORT
}

// see: cans/07-wire-plane.md#Synthetic-DNS-gateway
pub fn dns_payload(frame: &[u8], direction: FrameDirection) -> Option<&[u8]> {
    if !is_synthetic_dns(frame, direction) {
        return None;
    }
    let ihl = usize::from(frame[14] & 0x0f) * 4;
    let l4_start = 14 + ihl;
    let protocol = frame[14 + 9];
    let payload_start = match protocol {
        17 => {
            if frame.len() < l4_start + 8 {
                return None;
            }
            l4_start + 8
        }
        6 => {
            if frame.len() < l4_start + 13 {
                return None;
            }
            let tcp_hlen = usize::from(frame[l4_start + 12] >> 4) * 4;
            if tcp_hlen < 20 || frame.len() < l4_start + tcp_hlen {
                return None;
            }
            l4_start + tcp_hlen
        }
        _ => return None,
    };
    Some(&frame[payload_start..])
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
    metrics: Arc<kelir_otlp::registry::MetricRegistry>,
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
            metrics: Arc::new(kelir_otlp::registry::MetricRegistry::new()),
        }
    }

    pub fn with_metrics(mut self, metrics: Arc<kelir_otlp::registry::MetricRegistry>) -> Self {
        self.metrics = metrics;
        self
    }

    pub fn metrics(&self) -> &Arc<kelir_otlp::registry::MetricRegistry> {
        &self.metrics
    }

    fn record_drop(&self, action: &kelir_interface::protocol::FilterAction) {
        if matches!(action, kelir_interface::protocol::FilterAction::Drop { .. }) {
            self.metrics.record_drop();
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
        let action: Result<
            kelir_interface::protocol::FilterAction,
            kelir_interface::plugin::PluginError,
        > = match classify_frame_directional(frame, FrameDirection::Inbound) {
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
        };
        let action = action?;
        self.record_drop(&action);
        Ok(action)
    }

    pub fn process_outbound_frame(
        &mut self,
        frame: &mut [u8],
    ) -> Result<kelir_interface::protocol::FilterAction, kelir_interface::plugin::PluginError> {
        let action: Result<
            kelir_interface::protocol::FilterAction,
            kelir_interface::plugin::PluginError,
        > = match classify_frame_directional(frame, FrameDirection::Outbound) {
            FrameClass::TooShort
            | FrameClass::NonIp
            | FrameClass::NonTcpUdp
            | FrameClass::UdpOther => Ok(FilterAction::Drop {
                errno: None,
                reason: "non-TCP/UDP frame dropped at egress ring".into(),
            }),
            FrameClass::TcpDns | FrameClass::UdpDns => {
                match dns_payload(frame, FrameDirection::Outbound) {
                    Some(payload) => {
                        // TODO (L3): answer via resolve_synthetic_dns once the DNS wire parser lands.
                        let _ = self.resolve_synthetic_dns(payload)?;
                        Ok(FilterAction::Forward)
                    }
                    None => Ok(FilterAction::Drop {
                        errno: None,
                        reason: "DNS frame without a gateway payload dropped at egress ring".into(),
                    }),
                }
            }
            FrameClass::Tcp => {
                // TODO (L3): run policy + NAT over the outbound TCP stream via the smoltcp stack.
                let _ = self.stack.poll()?;
                Ok(FilterAction::Forward)
            }
        };
        let action = action?;
        self.record_drop(&action);
        Ok(action)
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
        payload: &[u8],
    ) -> Result<DnsResponse, kelir_interface::plugin::PluginError> {
        // TODO (L3): parse the query, apply the domain allowlist, answer from host resolver state via the synthetic gateway, EDNS0-downgrade the response.
        let _ = (payload, &self.allowed_domains, &self.synthetic_dns_ip);
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
