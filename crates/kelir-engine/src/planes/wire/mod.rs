// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#Userspace-network-stack
// see: cans/07-wire-plane.md#Synthetic-DNS-gateway
use std::sync::{Arc, Mutex, RwLock};

use kelir_interface::plugin_model::PluginError;
use kelir_interface::protocol_wire::FilterAction;
use kelir_interface::vsock_mesh::VsockServiceHandler;
use kelir_interface::wire::dns::{DnsOps, DnsQuery, DnsResponse};
use kelir_interface::wire::l4::{L4DivertOps, L4Stream};
use kelir_interface::wire::net::{
    FrameDirection, LinkLocalRoute, NetOps, WireWiring, link_local_route,
};
use kelir_interface::wire::udp_nat::UdpNatOps;

#[path = "layout_wire.flow.rs"]
pub(crate) mod layout_wire;

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
#[path = "plane.flow.rs"]
mod plane;
#[path = "unbound_ops.flow.rs"]
mod unbound_ops;

use unbound_ops::{UnboundDnsOps, UnboundL4DivertOps, UnboundNetOps, UnboundUdpNatOps};

pub struct WirePlane {
    net_ops: Arc<dyn NetOps>,
    dns_ops: Arc<dyn DnsOps>,
    udp_nat_ops: Arc<dyn UdpNatOps>,
    l4_ops: Arc<dyn L4DivertOps>,
    port_forwards: Arc<Mutex<std::collections::HashMap<u16, u16>>>,
    host_listeners:
        Arc<Mutex<std::collections::HashMap<u16, kelir_transport::host_bind::HostListener>>>,
    imds_sink: Arc<RwLock<Option<Arc<dyn VsockServiceHandler>>>>,
    metrics: Arc<kelir_otlp::registry_otlp::MetricRegistry>,
    wiring: Option<WireWiring>,
}
// see: cans/07-wire-plane.md#Wire-architecture
pub type WireOps = (
    Arc<dyn NetOps>,
    Arc<dyn DnsOps>,
    Arc<dyn UdpNatOps>,
    Arc<dyn L4DivertOps>,
);
