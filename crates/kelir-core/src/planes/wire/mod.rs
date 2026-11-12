// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#Userspace-network-stack
// see: cans/07-wire-plane.md#Synthetic-DNS-gateway
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex, RwLock};

use kelir_interface::errno_types::Errno;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::protocol_wire::{FilterAction, StreamKind};
use kelir_interface::vsock_mesh::{VAULT_METADATA_VSOCK_PORT, VsockServiceHandler};

pub(crate) mod layout_wire;

pub const SYNTHETIC_DNS_GATEWAY_IP: Ipv4Addr = Ipv4Addr::new(192, 168, 127, 1);
pub const SYNTHETIC_DNS_PORT: u16 = 53;

// see: cans/07-wire-plane.md#Synthetic-DNS-gateway
pub const RESOLV_CONF_PATH: &str = "/etc/resolv.conf";

// see: cans/07-wire-plane.md#Port-forwarding
#[derive(Debug, thiserror::Error)]
pub enum WireError {
    #[error("host port {0} already forwarded (EADDRINUSE)")]
    PortOccupied(u16),
}

impl From<WireError> for PluginError {
    fn from(error: WireError) -> Self {
        PluginError::Halt(error.to_string())
    }
}

// see: cans/07-wire-plane.md#Pre-policy-drop-zones
pub const LINK_LOCAL_IMDS_IP: Ipv4Addr = Ipv4Addr::new(169, 254, 169, 254);

pub struct SmoltcpStack {
    interface: Option<smoltcp::iface::Interface>,
}

impl SmoltcpStack {
    pub fn new() -> Self {
        Self { interface: None }
    }

    pub fn poll(&mut self) -> Result<bool, PluginError> {
        // TODO (L3) [vr_11_virtio_net_smoltcp_frame_sink]: attach the virtio-net device + SocketSet and run interface.poll over the TCP/UDP sockets; slop: host TAP created, frames never sink into smoltcp
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

// see: cans/07-wire-plane.md#Synthetic-DNS-gateway
// TODO (L3) [wp_09_dns_gateway_host_state_tracking]: reload nameservers via inotify when /etc/resolv.conf changes; slop: resolv.conf change ignored, stale upstream kept
fn host_resolver_nameservers() -> Vec<IpAddr> {
    std::fs::read_to_string(RESOLV_CONF_PATH)
        .map(|text| parse_resolv_conf_nameservers(&text))
        .unwrap_or_default()
}

// see: cans/07-wire-plane.md#Synthetic-DNS-gateway
fn parse_resolv_conf_nameservers(text: &str) -> Vec<IpAddr> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            match (fields.next(), fields.next()) {
                (Some("nameserver"), Some(addr)) => addr.parse().ok(),
                _ => None,
            }
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct DnsQuery {
    pub name: String,
    pub qtype: u16,
    pub edns_do: bool,
}

impl DnsQuery {
    pub fn parse(bytes: &[u8]) -> Result<Self, PluginError> {
        // TODO (L3) [wp_02_synthetic_dns_nxdomain_enforcement]: parse the DNS wire query (name, qtype, EDNS0 OPT record); slop: query forwarded upstream, ServFail not synthetic NXDOMAIN
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

impl DnsResponse {
    // see: cans/07-wire-plane.md#EDNS0-downgrade
    pub fn downgrade_edns0(&mut self) {
        // TODO (L3) [wire_plane]: clear the AD flag bit and the OPT record DO bit inside wire_bytes; slop: AD/DO bits leak upstream, downgrade never applied
        self.edns_downgraded = true;
    }
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
    if frame.len() < layout_wire::ETH_HEADER_BYTES {
        return None;
    }
    if layout_wire::EthView::new_unchecked(frame).etype() != layout_wire::ETHERTYPE_IPV4 {
        return None;
    }
    let ip = layout_wire::Ipv4View::new_unchecked(&frame[layout_wire::ETH_HEADER_BYTES..]);
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
    if frame.len() < layout_wire::ETH_HEADER_BYTES {
        return FrameClass::TooShort;
    }
    if layout_wire::EthView::new_unchecked(frame).etype() != layout_wire::ETHERTYPE_IPV4 {
        return FrameClass::NonIp;
    }
    let ip = layout_wire::Ipv4View::new_unchecked(&frame[layout_wire::ETH_HEADER_BYTES..]);
    let Some(ihl) = ip.ihl_bytes_checked() else {
        return FrameClass::TooShort;
    };
    if ihl < layout_wire::IPV4_MIN_HEADER_BYTES
        || frame.len() < layout_wire::ETH_HEADER_BYTES + ihl + layout_wire::L4_PORT_PAIR_BYTES
    {
        return FrameClass::TooShort;
    }
    match ip.proto() {
        layout_wire::IP_PROTO_TCP => {
            if is_synthetic_dns(frame, direction) {
                FrameClass::TcpDns
            } else {
                FrameClass::Tcp
            }
        }
        layout_wire::IP_PROTO_UDP => {
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
    if frame.len() < layout_wire::ETH_HEADER_BYTES {
        return false;
    }
    if layout_wire::EthView::new_unchecked(frame).etype() != layout_wire::ETHERTYPE_IPV4 {
        return false;
    }
    let ip = layout_wire::Ipv4View::new_unchecked(&frame[layout_wire::ETH_HEADER_BYTES..]);
    let Some(ihl) = ip.ihl_bytes_checked() else {
        return false;
    };
    if ihl < layout_wire::IPV4_MIN_HEADER_BYTES
        || frame.len() < layout_wire::ETH_HEADER_BYTES + ihl + layout_wire::L4_PORT_PAIR_BYTES
    {
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
    if protocol != layout_wire::IP_PROTO_TCP && protocol != layout_wire::IP_PROTO_UDP {
        return false;
    }
    let l4_start = layout_wire::ETH_HEADER_BYTES + ihl;
    let port = match protocol {
        layout_wire::IP_PROTO_TCP => {
            let tcp = layout_wire::TcpView::new_unchecked(&frame[l4_start..]);
            match direction {
                FrameDirection::Inbound => tcp.src_port(),
                FrameDirection::Outbound => tcp.dst_port(),
            }
        }
        _ => {
            let udp = layout_wire::UdpView::new_unchecked(&frame[l4_start..]);
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
    let ip = layout_wire::Ipv4View::new_unchecked(&frame[layout_wire::ETH_HEADER_BYTES..]);
    let l4_start = layout_wire::ETH_HEADER_BYTES + ip.ihl_bytes_checked()?;
    match ip.proto() {
        layout_wire::IP_PROTO_UDP => {
            if frame.len() < l4_start + layout_wire::UDP_HEADER_BYTES {
                return None;
            }
            Some(&frame[l4_start + layout_wire::UDP_HEADER_BYTES..])
        }
        layout_wire::IP_PROTO_TCP => {
            let tcp = layout_wire::TcpView::new_unchecked(&frame[l4_start..]);
            let header_bytes = tcp.header_bytes_checked()?;
            if header_bytes < layout_wire::TCP_MIN_HEADER_BYTES
                || frame.len() < l4_start + header_bytes
            {
                return None;
            }
            Some(&frame[l4_start + header_bytes..])
        }
        _ => None,
    }
}

// see: cans/07-wire-plane.md#DNS-policy-filter
// see: cans/07-wire-plane.md#Synthetic-DNS-gateway
fn domain_matches(domains: &[String], name: &str) -> bool {
    domains.iter().any(|entry| {
        entry == name
            || entry == "*"
            || name
                .strip_suffix(entry.as_str())
                .is_some_and(|head| head.ends_with('.'))
    })
}

pub struct UdpNatEntry {
    guest_source_port: u16,
    last_active_unix: u64,
}

// see: cans/07-wire-plane.md#UDP-socket-reaper
pub const UDP_NAT_SLAB_CAPACITY: usize = 1_024;

// see: cans/07-wire-plane.md#UDP-socket-reaper
pub const UDP_NAT_IDLE_TIMEOUT_SECS: u64 = 30;

pub struct WirePlane {
    synthetic_dns_ip: Ipv4Addr,
    resolver_nameservers: Vec<IpAddr>,
    udp_nat_slab: Arc<Mutex<Vec<Option<UdpNatEntry>>>>,
    port_forwards: Arc<Mutex<std::collections::HashMap<u16, u16>>>,
    host_listeners:
        Arc<Mutex<std::collections::HashMap<u16, kelir_transport::host_bind::HostListener>>>,
    allowed_domains: Arc<RwLock<Vec<String>>>,
    stack: SmoltcpStack,
    imds_sink: Arc<RwLock<Option<Arc<dyn VsockServiceHandler>>>>,
    metrics: Arc<kelir_otlp::registry_otlp::MetricRegistry>,
    wiring: Option<WireWiring>,
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

impl Default for WirePlane {
    fn default() -> Self {
        Self::new()
    }
}

impl WirePlane {
    pub fn new() -> Self {
        Self {
            synthetic_dns_ip: SYNTHETIC_DNS_GATEWAY_IP,
            resolver_nameservers: host_resolver_nameservers(),
            udp_nat_slab: Arc::new(Mutex::new(Vec::new())),
            port_forwards: Arc::new(Mutex::new(std::collections::HashMap::new())),
            host_listeners: Arc::new(Mutex::new(std::collections::HashMap::new())),
            allowed_domains: Arc::new(RwLock::new(Vec::new())),
            stack: SmoltcpStack::new(),
            imds_sink: Arc::new(RwLock::new(None)),
            metrics: Arc::new(kelir_otlp::registry_otlp::MetricRegistry::new()),
            wiring: None,
        }
    }

    // see: cans/07-wire-plane.md#DNS-policy-filter
    // TODO (L3) [wire_plane]: the ssh agent bridge mounts, vault placeholders hydrate, and l4 proxies open host listeners; slop: agent bridge unmounted, placeholders dry, listeners closed
    pub fn with_wiring(self, wiring: WireWiring) -> Self {
        if let Ok(mut domains) = self.allowed_domains.write() {
            *domains = wiring.sinkhole_allowlist.clone();
        }
        Self {
            wiring: Some(wiring),
            ..self
        }
    }

    pub fn with_metrics(mut self, metrics: Arc<kelir_otlp::registry_otlp::MetricRegistry>) -> Self {
        self.metrics = metrics;
        self
    }

    pub fn metrics(&self) -> &Arc<kelir_otlp::registry_otlp::MetricRegistry> {
        &self.metrics
    }

    // see: cans/07-wire-plane.md#DNS-policy-filter
    pub fn wiring(&self) -> Option<&WireWiring> {
        self.wiring.as_ref()
    }

    // see: cans/07-wire-plane.md#Pre-policy-drop-zones
    pub fn bind_imds_sink(&self, handler: Arc<dyn VsockServiceHandler>) -> Result<(), PluginError> {
        match self.imds_sink.write() {
            Ok(mut guard) => {
                *guard = Some(handler);
                Ok(())
            }
            Err(_) => Err(PluginError::Halt("imds sink slot poisoned".into())),
        }
    }

    pub fn imds_sink_bound(&self) -> bool {
        matches!(self.imds_sink.read(), Ok(guard) if guard.is_some())
    }

    pub fn imds_sink(&self) -> Result<Option<Arc<dyn VsockServiceHandler>>, PluginError> {
        match self.imds_sink.read() {
            Ok(guard) => Ok(guard.clone()),
            Err(_) => Err(PluginError::Halt("imds sink slot poisoned".into())),
        }
    }

    fn record_drop(&self, action: &kelir_interface::protocol_wire::FilterAction) {
        if matches!(
            action,
            kelir_interface::protocol_wire::FilterAction::Drop { .. }
        ) {
            self.metrics.record_drop();
        }
    }

    // see: cans/07-wire-plane.md#Port-forwarding
    pub fn forward_port(
        &self,
        guest_port: u16,
        host_port: Option<u16>,
    ) -> Result<u16, kelir_interface::plugin_model::PluginError> {
        let mut forwards = self.port_forwards.lock().map_err(|_| {
            kelir_interface::plugin_model::PluginError::Halt("port forward table poisoned".into())
        })?;
        let mut listeners = self.host_listeners.lock().map_err(|_| {
            kelir_interface::plugin_model::PluginError::Halt("host listener table poisoned".into())
        })?;
        let bind_port = match host_port.filter(|port| *port != 0) {
            Some(explicit) => {
                if forwards.contains_key(&explicit) {
                    return Err(WireError::PortOccupied(explicit).into());
                }
                explicit
            }
            None => 0,
        };
        let listener =
            kelir_transport::host_bind::bind_host_listener(bind_port).map_err(|error| {
                if bind_port != 0 && error.kind() == std::io::ErrorKind::AddrInUse {
                    WireError::PortOccupied(bind_port).into()
                } else {
                    kelir_interface::plugin_model::PluginError::Halt(format!(
                        "host port {bind_port} bind: {error}"
                    ))
                }
            })?;
        let bound = listener.port();
        forwards.insert(bound, guest_port);
        listeners.insert(bound, listener);
        Ok(bound)
    }

    // see: cans/07-wire-plane.md#Port-forwarding
    pub fn forwarded_guest_port(&self, host_port: u16) -> Result<Option<u16>, PluginError> {
        let forwards = self
            .port_forwards
            .lock()
            .map_err(|_| PluginError::Halt("port forward table poisoned".into()))?;
        Ok(forwards.get(&host_port).copied())
    }

    pub fn process_inbound_frame(
        &mut self,
        frame: &mut [u8],
    ) -> Result<
        kelir_interface::protocol_wire::FilterAction,
        kelir_interface::plugin_model::PluginError,
    > {
        if let Some(LinkLocalRoute::Drop) = link_local_route(frame, FrameDirection::Inbound) {
            let action = FilterAction::Drop {
                errno: None,
                reason: "link-local 169.254.0.0/16 quarantined pre-policy".into(),
            };
            self.record_drop(&action);
            return Ok(action);
        }
        let action: Result<
            kelir_interface::protocol_wire::FilterAction,
            kelir_interface::plugin_model::PluginError,
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
                // TODO (L3) [vr_11_virtio_net_smoltcp_frame_sink]: terminate the TCP stream in the embedded smoltcp stack; slop: host TAP created, frames never sink into smoltcp
                let _ = self.stack.poll()?;
                Ok(FilterAction::Forward)
            }
        };
        let action = action?;
        self.record_drop(&action);
        Ok(self.surface_action(action))
    }

    pub fn process_outbound_frame(
        &mut self,
        frame: &mut [u8],
    ) -> Result<
        kelir_interface::protocol_wire::FilterAction,
        kelir_interface::plugin_model::PluginError,
    > {
        match link_local_route(frame, FrameDirection::Outbound) {
            Some(LinkLocalRoute::Drop) => {
                let action = FilterAction::Drop {
                    errno: None,
                    reason: "link-local 169.254.0.0/16 quarantined pre-policy".into(),
                };
                self.record_drop(&action);
                return Ok(action);
            }
            Some(LinkLocalRoute::VaultImds { .. }) => return self.route_imds_to_vault(frame),
            None => {}
        }
        let action: Result<
            kelir_interface::protocol_wire::FilterAction,
            kelir_interface::plugin_model::PluginError,
        > = match classify_frame_directional(frame, FrameDirection::Outbound) {
            FrameClass::TooShort | FrameClass::NonIp | FrameClass::NonTcpUdp => {
                Ok(FilterAction::Drop {
                    errno: None,
                    reason: "non-TCP/UDP frame dropped at egress ring".into(),
                })
            }
            // see: cans/07-wire-plane.md#UDP-NAT-semantics
            FrameClass::UdpOther => {
                if self
                    .wiring
                    .as_ref()
                    .is_some_and(|wiring| wiring.allow_udp_proxied)
                {
                    // TODO (L3) [wire_plane]: forward via the full-cone UDP NAT host socket per guest source port; slop: UDP forwarded without per-port host socket
                    let _ = self.stack.poll()?;
                    Ok(FilterAction::Forward)
                } else {
                    Ok(FilterAction::Drop {
                        errno: Some(Errno::Econnrefused),
                        reason: "UDP egress denied: allow_udp_proxied not enabled".into(),
                    })
                }
            }
            FrameClass::TcpDns | FrameClass::UdpDns => {
                match dns_payload(frame, FrameDirection::Outbound) {
                    Some(payload) => {
                        // TODO (L3) [wp_02_synthetic_dns_nxdomain_enforcement]: synthesize the answer frame into the guest ingress ring once the DNS wire parser lands; slop: query forwarded upstream, ServFail not synthetic NXDOMAIN
                        self.resolve_synthetic_dns(payload)?;
                        Ok(FilterAction::Forward)
                    }
                    None => Ok(FilterAction::Drop {
                        errno: None,
                        reason: "DNS frame without a gateway payload dropped at egress ring".into(),
                    }),
                }
            }
            FrameClass::Tcp => {
                // TODO (L3) [wp_04_userspace_netstack_unprivileged_dial]: run policy + NAT over the outbound TCP stream via the smoltcp stack; slop: dial needs CAP_NET_ADMIN or iptables, not plain socket
                let _ = self.stack.poll()?;
                Ok(FilterAction::Forward)
            }
        };
        let action = action?;
        self.record_drop(&action);
        Ok(self.surface_action(action))
    }

    // see: cans/07-wire-plane.md#Frame-drop-and-forward-semantics
    // TODO (L3) [wire_plane]: vCPU loop must act on this; slop: surfaced action never acted on, vCPU ignores verdict
    pub fn surface_action(
        &self,
        action: kelir_interface::protocol_wire::FilterAction,
    ) -> kelir_interface::protocol_wire::FilterAction {
        if matches!(
            action,
            kelir_interface::protocol_wire::FilterAction::Suspend { .. }
        ) {
            tracing::warn!("FilterAction::Suspend returned without consumer");
        }
        action
    }

    // see: cans/07-wire-plane.md#Pre-policy-drop-zones
    pub fn route_imds_to_vault(
        &self,
        frame: &[u8],
    ) -> Result<
        kelir_interface::protocol_wire::FilterAction,
        kelir_interface::plugin_model::PluginError,
    > {
        match link_local_route(frame, FrameDirection::Outbound) {
            Some(LinkLocalRoute::VaultImds { vsock_port }) => {
                if self.imds_sink()?.is_none() {
                    return Err(PluginError::Halt(format!(
                        "IMDS 169.254.169.254:{vsock_port} has no vault vsock sink bound; mount plugin-vault and bind it"
                    )));
                }
                // TODO (L3) [wire_plane]: parse the HTTP metadata request and bridge it to the bound vault vsock service stream; slop: metadata request never bridged to vault vsock
                Ok(kelir_interface::protocol_wire::FilterAction::Forward)
            }
            _ => Err(PluginError::Halt(
                "route_imds_to_vault called on a non-IMDS frame".into(),
            )),
        }
    }

    pub fn divert_l4(&self, guest_port: u16, host_target: String) -> Result<L4Stream, PluginError> {
        // TODO (L3) [wp_12_l4_diversion_passthrough_tls]: divert the guest TCP stream over stream kind 0x0005 to the host target; slop: TLS terminated in supervisor, plaintext in memory
        let _ = L4Stream::new(guest_port, host_target);
        Err(PluginError::Halt(
            "L4 stream diversion over 0x0005 pending (L3)".into(),
        ))
    }

    // see: cans/07-wire-plane.md#Synthetic-DNS-gateway
    // see: cans/07-wire-plane.md#DNS-policy-filter
    pub fn resolve_query(
        &self,
        query: &mut DnsQuery,
    ) -> Result<DnsResponse, kelir_interface::plugin_model::PluginError> {
        let blocked = {
            let allowed = self
                .allowed_domains
                .read()
                .map_err(|_| PluginError::Halt("dns allowlist lock poisoned".into()))?;
            self.wiring
                .as_ref()
                .is_some_and(|wiring| wiring.sinkhole_drop_all)
                || (!allowed.is_empty() && !domain_matches(&allowed, &query.name))
        };
        if blocked {
            // TODO (L3) [wp_02_synthetic_dns_nxdomain_enforcement]: serialize the synthetic NXDOMAIN answer into wire_bytes; slop: query forwarded upstream, ServFail not synthetic NXDOMAIN
            let mut response = DnsResponse {
                wire_bytes: Vec::new(),
                rcode: DnsRcode::NxDomain,
                edns_downgraded: false,
            };
            response.downgrade_edns0();
            return Ok(response);
        }
        // see: cans/07-wire-plane.md#EDNS0-downgrade
        query.downgrade_edns0();
        let mut response = DnsResponse {
            wire_bytes: Vec::new(),
            rcode: DnsRcode::ServFail,
            edns_downgraded: false,
        };
        response.downgrade_edns0();
        // TODO (L3) [wp_09_dns_gateway_host_state_tracking]: relay the query to the live upstream resolver; ServFail is the answer until the relay is wired; slop: resolv.conf change ignored, stale upstream kept
        Ok(response)
    }

    pub fn resolve_synthetic_dns(
        &self,
        payload: &[u8],
    ) -> Result<DnsResponse, kelir_interface::plugin_model::PluginError> {
        // TODO (L3) [wp_09_dns_gateway_host_state_tracking]: parse the query, apply the domain allowlist, forward to the tracked nameservers, EDNS0-downgrade the response; slop: resolv.conf change ignored, stale upstream kept
        let _ = (payload, &self.allowed_domains, &self.synthetic_dns_ip);
        if self.resolver_nameservers.is_empty() {
            return Err(PluginError::Halt(
                "DNS resolver pending (L3): no nameservers tracked from /etc/resolv.conf".into(),
            ));
        }
        Err(PluginError::Halt(
            "DNS resolver pending (L3): query parse and upstream forward".into(),
        ))
    }

    // see: cans/07-wire-plane.md#UDP-socket-reaper
    pub fn udp_nat_alloc(
        &self,
        guest_source_port: u16,
        now_unix: u64,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        let mut slab = self.udp_nat_slab.lock().map_err(|_| {
            kelir_interface::plugin_model::PluginError::Halt("udp nat slab lock poisoned".into())
        })?;
        if let Some(slot) = slab
            .iter_mut()
            .flatten()
            .find(|entry| entry.guest_source_port == guest_source_port)
        {
            slot.last_active_unix = now_unix;
            return Ok(());
        }
        // TODO (L3) [wire_plane]: open the per-source-port host UDP socket and store its descriptor in the slab entry (Full Cone endpoint independence); slop: slab entry without host socket, endpoint independence broken
        let entry = UdpNatEntry {
            guest_source_port,
            last_active_unix: now_unix,
        };
        if let Some(free) = slab.iter().position(Option::is_none) {
            slab[free] = Some(entry);
            return Ok(());
        }
        if slab.len() < UDP_NAT_SLAB_CAPACITY {
            slab.push(Some(entry));
            return Ok(());
        }
        let oldest = slab.iter().enumerate().min_by_key(|(_, slot)| {
            slot.as_ref()
                .map_or(u64::MAX, |entry| entry.last_active_unix)
        });
        let Some((index, _)) = oldest else {
            return Err(PluginError::Halt("udp nat slab cap exhausted".into()));
        };
        slab[index] = Some(entry);
        Ok(())
    }

    // see: cans/07-wire-plane.md#UDP-socket-reaper
    pub fn udp_nat_slab_used(&self) -> Result<usize, PluginError> {
        let slab = self
            .udp_nat_slab
            .lock()
            .map_err(|_| PluginError::Halt("udp nat slab lock poisoned".into()))?;
        Ok(slab.iter().flatten().count())
    }

    // see: cans/07-wire-plane.md#UDP-socket-reaper
    pub fn reap_stale_udp_sockets(
        &self,
        now_unix: u64,
    ) -> Result<usize, kelir_interface::plugin_model::PluginError> {
        let mut slab = self.udp_nat_slab.lock().map_err(|_| {
            kelir_interface::plugin_model::PluginError::Halt("udp nat slab lock poisoned".into())
        })?;
        let mut reaped = 0usize;
        for slot in slab.iter_mut() {
            let Some(entry) = slot.as_ref() else {
                continue;
            };
            if now_unix.saturating_sub(entry.last_active_unix) > UDP_NAT_IDLE_TIMEOUT_SECS {
                // TODO (L3) [wire_plane]: close(2) the host UDP socket descriptor before releasing the slab slot; slop: slab released with host fd open, descriptor leaks
                *slot = None;
                reaped += 1;
            }
        }
        Ok(reaped)
    }

    // see: cans/08-lifecycle.md#Rollback
    // see: cans/07-wire-plane.md#Rollback-interaction
    // see: cans/07-wire-plane.md#Socket-lifecycle
    pub fn quiesce(&self) -> Result<(), PluginError> {
        // TODO (L3) [wp_03_rollback_socket_termination_econnreset]: close live host-side L4 sockets with SO_LINGER zero, emit RST to peers, drain splicing buffers; slop: rollback Ok, host sockets open, no RST, send succeeds
        let mut slab = self
            .udp_nat_slab
            .lock()
            .map_err(|_| PluginError::Halt("udp nat slab lock poisoned".into()))?;
        slab.clear();
        let mut forwards = self
            .port_forwards
            .lock()
            .map_err(|_| PluginError::Halt("port forward table poisoned".into()))?;
        forwards.clear();
        let mut listeners = self
            .host_listeners
            .lock()
            .map_err(|_| PluginError::Halt("host listener table poisoned".into()))?;
        listeners.clear();
        Ok(())
    }
}
