// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#Userspace-network-stack
use std::sync::{Arc, Mutex};

use kelir_engine::dispatch_plugin::PipelineEngine;
use kelir_interface::errno_types::Errno;
use kelir_interface::frame_layout_views::{
    ETH_HEADER_BYTES, ETHERTYPE_IPV4, EthView, IP_PROTO_TCP, IPV4_MIN_HEADER_BYTES, Ipv4View,
    TcpView,
};
use kelir_interface::plugin_model::PluginError;
use kelir_interface::protocol_wire::{FilterAction, Verdict};
use kelir_interface::wire::dns::DnsOps;
use kelir_interface::wire::net::{
    FrameClass, FrameDirection, LinkLocalRoute, NetOps, WireWiring, classify_frame_directional,
    dns_payload, link_local_route,
};

pub(crate) struct SmoltcpStack {
    interface: Option<smoltcp::iface::Interface>,
}

impl SmoltcpStack {
    pub(crate) fn new() -> Self {
        Self { interface: None }
    }

    pub(crate) fn poll(&mut self) -> Result<bool, PluginError> {
        // L3-GAP: [vr_11_virtio_net_smoltcp_frame_sink]: attach the virtio-net device + SocketSet and run interface.poll over the TCP/UDP sockets; slop: host TAP created, frames never sink into smoltcp
        let _ = &self.interface;
        Ok(false)
    }
}

impl Default for SmoltcpStack {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) struct RealNetOps {
    engine: Arc<PipelineEngine>,
    stack: Mutex<SmoltcpStack>,
    dns_ops: Arc<dyn DnsOps>,
    allow_udp_proxied: Mutex<bool>,
}

impl RealNetOps {
    pub(crate) fn new(engine: Arc<PipelineEngine>, dns_ops: Arc<dyn DnsOps>) -> Self {
        Self {
            engine,
            stack: Mutex::new(SmoltcpStack::new()),
            dns_ops,
            allow_udp_proxied: Mutex::new(false),
        }
    }

    fn poll_stack(&self) -> Result<bool, PluginError> {
        let mut stack = self
            .stack
            .lock()
            .map_err(|_| PluginError::Halt("smoltcp stack lock poisoned".into()))?;
        stack.poll()
    }

    fn intercept_inbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        match classify_frame_directional(frame, FrameDirection::Inbound) {
            FrameClass::TooShort
            | FrameClass::NonIp
            | FrameClass::NonTcpUdp
            | FrameClass::UdpOther => Ok(FilterAction::Drop {
                errno: Errno::Eperm,
                reason: "non-TCP/UDP frame dropped at ingress ring".into(),
            }),
            FrameClass::TcpDns | FrameClass::UdpDns => Ok(FilterAction::Forward),
            FrameClass::Tcp => {
                // see: cans/07-wire-plane.md#Hook-points:-on-net-outbound,-on-net-inbound,-on-socket-connect
                if let Verdict::Drop { errno, reason } = self.engine.dispatch_socket_connect(
                    0,
                    IP_PROTO_TCP,
                    remote_ipv4(frame).unwrap_or([0, 0, 0, 0]),
                    remote_port(frame).unwrap_or(0),
                )? {
                    return Ok(FilterAction::Drop { errno, reason });
                }
                // L3-GAP: [vr_11_virtio_net_smoltcp_frame_sink]: terminate the TCP stream in the embedded smoltcp stack; slop: host TAP created, frames never sink into smoltcp
                let _ = self.poll_stack()?;
                Ok(FilterAction::Forward)
            }
        }
    }

    fn intercept_outbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        match classify_frame_directional(frame, FrameDirection::Outbound) {
            FrameClass::TooShort | FrameClass::NonIp | FrameClass::NonTcpUdp => {
                Ok(FilterAction::Drop {
                    errno: Errno::Eperm,
                    reason: "non-TCP/UDP frame dropped at egress ring".into(),
                })
            }
            // see: cans/07-wire-plane.md#UDP-NAT-semantics
            FrameClass::UdpOther => {
                let allow = self
                    .allow_udp_proxied
                    .lock()
                    .map_err(|_| PluginError::Halt("udp proxy flag lock poisoned".into()))?;
                if *allow {
                    // L3-GAP: [wire_plane]: forward via the full-cone UDP NAT host socket per guest source port; slop: UDP forwarded without per-port host socket
                    let _ = self.poll_stack()?;
                    Ok(FilterAction::Forward)
                } else {
                    Ok(FilterAction::Drop {
                        errno: Errno::Econnrefused,
                        reason: "UDP egress denied: allow_udp_proxied not enabled".into(),
                    })
                }
            }
            FrameClass::TcpDns | FrameClass::UdpDns => {
                // see: cans/07-wire-plane.md#on-dns-query-mediates-domain-resolution-before-packet-egress
                // TODO (L3) [wire_plane]: parse the DNS wire query name/qtype and resolve the guest PID instead of the stub ring call; slop: dns-query ring runs on an empty name, per-PID DNS policy blind
                if matches!(
                    self.engine.dispatch_dns_query(0, "", 0)?,
                    kelir_interface::wire::dns::DnsVerdict::Sinkhole { .. }
                ) {
                    return Ok(FilterAction::Drop {
                        errno: Errno::Eperm,
                        reason: "DNS query denied by dns-query ring".into(),
                    });
                }
                match dns_payload(frame, FrameDirection::Outbound) {
                    Some(payload) => {
                        // L3-GAP: [wp_02_synthetic_dns_nxdomain_enforcement]: synthesize the answer frame into the guest ingress ring once the DNS wire parser lands; slop: query forwarded upstream, ServFail not synthetic NXDOMAIN
                        self.dns_ops.resolve_synthetic_dns(payload)?;
                        Ok(FilterAction::Forward)
                    }
                    None => Ok(FilterAction::Drop {
                        errno: Errno::Eperm,
                        reason: "DNS frame without a gateway payload dropped at egress ring".into(),
                    }),
                }
            }
            FrameClass::Tcp => {
                // L3-GAP: [wp_04_userspace_netstack_unprivileged_dial]: run policy + NAT over the outbound TCP stream via the smoltcp stack; slop: dial needs CAP_NET_ADMIN or iptables, not plain socket
                let _ = self.poll_stack()?;
                Ok(FilterAction::Forward)
            }
        }
    }
}

pub(crate) fn build_net_ops(
    engine: Arc<PipelineEngine>,
    dns_ops: Arc<dyn DnsOps>,
) -> Arc<dyn NetOps> {
    Arc::new(RealNetOps::new(engine, dns_ops))
}

// see: cans/07-wire-plane.md#Userspace-network-stack
// TODO (L3) [wire_plane]: resolve the connecting guest PID via the socket table instead of pid 0 on the socket-connect ring; slop: socket-connect ring runs with pid 0, PNACL enforcement blind
fn remote_ipv4(frame: &[u8]) -> Option<[u8; 4]> {
    if frame.len() < ETH_HEADER_BYTES || EthView::new_unchecked(frame).etype() != ETHERTYPE_IPV4 {
        return None;
    }
    let ip = Ipv4View::new_unchecked(&frame[ETH_HEADER_BYTES..]);
    let ihl = ip.ihl_bytes_checked()?;
    if ihl < IPV4_MIN_HEADER_BYTES {
        return None;
    }
    ip.src_checked()
}

// see: cans/07-wire-plane.md#Userspace-network-stack
fn remote_port(frame: &[u8]) -> Option<u16> {
    if frame.len() < ETH_HEADER_BYTES || EthView::new_unchecked(frame).etype() != ETHERTYPE_IPV4 {
        return None;
    }
    let ip = Ipv4View::new_unchecked(&frame[ETH_HEADER_BYTES..]);
    let ihl = ip.ihl_bytes_checked()?;
    if ihl < IPV4_MIN_HEADER_BYTES || ip.proto() != IP_PROTO_TCP {
        return None;
    }
    let tcp = TcpView::new_unchecked(&frame[ETH_HEADER_BYTES + ihl..]);
    Some(tcp.src_port())
}

impl NetOps for RealNetOps {
    fn intercept_ethernet_frame(
        &self,
        direction: FrameDirection,
        frame: &mut [u8],
    ) -> Result<FilterAction, PluginError> {
        match link_local_route(frame, direction) {
            Some(LinkLocalRoute::Drop) => {
                return Ok(FilterAction::Drop {
                    errno: Errno::Eperm,
                    reason: "link-local 169.254.0.0/16 quarantined pre-policy".into(),
                });
            }
            None => {}
        }
        match direction {
            FrameDirection::Inbound => self.intercept_inbound(frame),
            FrameDirection::Outbound => self.intercept_outbound(frame),
        }
    }

    fn apply_wiring(&self, wiring: &WireWiring) {
        if let Ok(mut allow) = self.allow_udp_proxied.lock() {
            *allow = wiring.allow_udp_proxied;
        }
    }
}
