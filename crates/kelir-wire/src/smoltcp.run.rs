// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#Userspace-network-stack
use std::sync::{Arc, Mutex};

use kelir_engine::dispatch_plugin::PipelineEngine;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::protocol_wire::FilterAction;
use kelir_interface::wire::dns::DnsOps;
use kelir_interface::wire::net::{FrameDirection, NetOps, WireWiring};

// TODO (L3) [wire_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
pub(crate) struct SmoltcpStack {
    interface: Option<smoltcp::iface::Interface>,
}

// TODO (L3) [wire_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
impl SmoltcpStack {
    pub(crate) fn new() -> Self {
        Self { interface: None }
    }

    pub(crate) fn poll(&mut self) -> Result<bool, PluginError> {
        // L3-GAP: [vr_11_virtio_net_smoltcp_frame_sink]: attach the virtio-net device + SocketSet and run interface.poll over the TCP/UDP sockets; slop: host TAP created, frames never sink into smoltcp
        // TODO (L3) [wire_plane]: poll the smoltcp interface over its socket set and report whether it made progress.; slop: poll never runs, sockets stall while the stack reports progress.
        Err(PluginError::Halt(
            "TODO (L3): SmoltcpStack::poll needs the virtio-net device and socket set poll".into(),
        ))
    }
}

impl Default for SmoltcpStack {
    fn default() -> Self {
        Self::new()
    }
}

// TODO (L3) [wire_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
pub(crate) struct RealNetOps {
    engine: Arc<PipelineEngine>,
    stack: Mutex<SmoltcpStack>,
    dns_ops: Arc<dyn DnsOps>,
    allow_udp_proxied: Mutex<bool>,
}

// TODO (L3) [wire_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
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
        // TODO (L3) [wire_plane]: lock the smoltcp stack and poll it.; slop: stack never polled, inbound frames queue forever while poll reports success.
        Err(PluginError::Halt(
            "TODO (L3): RealNetOps::poll_stack needs the smoltcp stack poll".into(),
        ))
    }

    fn intercept_inbound(&self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        // L3-GAP: [vr_11_virtio_net_smoltcp_frame_sink]: terminate the TCP stream in the embedded smoltcp stack; slop: host TAP created, frames never sink into smoltcp
        // TODO (L3) [wire_plane]: classify the inbound frame, run the socket-connect ring, and sink TCP into smoltcp.; slop: inbound frames bypass the socket-connect ring, unvetted TCP reaches smoltcp.
        Err(PluginError::Halt(
            "TODO (L3): RealNetOps::intercept_inbound needs the inbound frame interception".into(),
        ))
    }

    fn intercept_outbound(&self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        // L3-GAP: [wire_plane]: forward via the full-cone UDP NAT host socket per guest source port; slop: UDP forwarded without per-port host socket
        // L3-GAP: [wp_02_synthetic_dns_nxdomain_enforcement]: synthesize the answer frame into the guest ingress ring once the DNS wire parser lands; slop: query forwarded upstream, ServFail not synthetic NXDOMAIN
        // L3-GAP: [wp_04_userspace_netstack_unprivileged_dial]: run policy + NAT over the outbound TCP stream via the smoltcp stack; slop: dial needs CAP_NET_ADMIN or iptables, not plain socket
        // TODO (L3) [wire_plane]: classify the outbound frame and run the DNS, UDP NAT and TCP policy paths.; slop: outbound frames skip DNS, UDP NAT and TCP policy, guest traffic leaves unvetted.
        Err(PluginError::Halt(
            "TODO (L3): RealNetOps::intercept_outbound needs the outbound frame interception"
                .into(),
        ))
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
// TODO (L3) [wire_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
fn remote_ipv4(_frame: &[u8]) -> Option<[u8; 4]> {
    // TODO (L3) [wire_plane]: parse the remote IPv4 address from the Ethernet frame.; slop: remote address stays zeroed, the socket-connect ring attributes the frame to the wrong peer.
    None
}

// see: cans/07-wire-plane.md#Userspace-network-stack
// TODO (L3) [wire_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
fn remote_port(_frame: &[u8]) -> Option<u16> {
    // TODO (L3) [wire_plane]: parse the remote TCP source port from the Ethernet frame.; slop: source port stays zeroed, the socket-connect ring attributes the frame to the wrong socket.
    None
}

impl NetOps for RealNetOps {
    fn intercept_ethernet_frame(
        &self,
        _direction: FrameDirection,
        _frame: &mut [u8],
    ) -> Result<FilterAction, PluginError> {
        // TODO (L3) [wire_plane]: quarantine link-local frames, then intercept the frame in direction.; slop: link-local frames reach the rings unquarantined, default-deny never enforced on ingress.
        Err(PluginError::Halt(
            "TODO (L3): NetOps::intercept_ethernet_frame needs the directional frame interception"
                .into(),
        ))
    }

    fn apply_wiring(&self, _wiring: &WireWiring) {
        // TODO (L3) [wire_plane]: store the allow_udp_proxied flag from wiring.; slop: flag stays at its default, UDP proxying runs opposite to the wired policy.
    }
}
