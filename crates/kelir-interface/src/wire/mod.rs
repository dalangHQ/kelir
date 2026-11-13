// see: cans/07-wire-plane.md#Wire-architecture
#[path = "dns_ops.rs"]
pub mod dns;
#[path = "l4_ops.rs"]
pub mod l4;
#[path = "net_ops.rs"]
pub mod net;
pub mod udp_nat;

pub use dns::{DnsOps, DnsQuery, DnsRcode, DnsResponse, RESOLV_CONF_PATH};
pub use l4::{L4DivertOps, L4Stream};
pub use net::{
    FrameClass, FrameDirection, LinkLocalRoute, NetOps, WireWiring, classify_frame,
    classify_frame_directional, dns_payload, is_synthetic_dns, link_local_route,
};
pub use udp_nat::{UDP_NAT_IDLE_TIMEOUT_SECS, UDP_NAT_SLAB_CAPACITY, UdpNatEntry, UdpNatOps};
