// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#Userspace-network-stack
use std::sync::{Arc, Mutex};

use kelir_interface::errno_types::Errno;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::protocol_wire::FilterAction;
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

pub(crate) struct RealNetOps {
    stack: Mutex<SmoltcpStack>,
    dns_ops: Arc<dyn DnsOps>,
    allow_udp_proxied: Mutex<bool>,
}

impl RealNetOps {
    pub(crate) fn new(dns_ops: Arc<dyn DnsOps>) -> Self {
        Self {
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
                errno: None,
                reason: "non-TCP/UDP frame dropped at ingress ring".into(),
            }),
            FrameClass::TcpDns | FrameClass::UdpDns => Ok(FilterAction::Forward),
            FrameClass::Tcp => {
                // TODO (L3) [vr_11_virtio_net_smoltcp_frame_sink]: terminate the TCP stream in the embedded smoltcp stack; slop: host TAP created, frames never sink into smoltcp
                let _ = self.poll_stack()?;
                Ok(FilterAction::Forward)
            }
        }
    }

    fn intercept_outbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        match classify_frame_directional(frame, FrameDirection::Outbound) {
            FrameClass::TooShort | FrameClass::NonIp | FrameClass::NonTcpUdp => {
                Ok(FilterAction::Drop {
                    errno: None,
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
                    // TODO (L3) [wire_plane]: forward via the full-cone UDP NAT host socket per guest source port; slop: UDP forwarded without per-port host socket
                    let _ = self.poll_stack()?;
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
                        self.dns_ops.resolve_synthetic_dns(payload)?;
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
                let _ = self.poll_stack()?;
                Ok(FilterAction::Forward)
            }
        }
    }
}

pub(crate) fn build_net_ops(dns_ops: Arc<dyn DnsOps>) -> Arc<dyn NetOps> {
    Arc::new(RealNetOps::new(dns_ops))
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
                    errno: None,
                    reason: "link-local 169.254.0.0/16 quarantined pre-policy".into(),
                });
            }
            Some(LinkLocalRoute::VaultImds { .. }) => {
                return Err(PluginError::Halt(
                    "IMDS frame reached NetOps without a bound vault sink; route via WirePlane::route_imds_to_vault"
                        .into(),
                ));
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
