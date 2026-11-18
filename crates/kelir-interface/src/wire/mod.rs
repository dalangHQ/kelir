// see: cans/07-wire-plane.md#Wire-architecture
#[path = "connection_parking.flow.rs"]
pub mod connection_parking;
#[path = "dns_ops.flow.rs"]
pub mod dns;
#[path = "l4_ops.rs"]
pub mod l4;
#[path = "l4_ops.flow.rs"]
pub mod l4_flow;
#[path = "net_ops.flow.rs"]
pub mod net;
#[path = "outbox.flow.rs"]
pub mod outbox;
#[path = "udp_nat.flow.rs"]
pub mod udp_nat;

pub use connection_parking::{ConnectionParking, ConnectionParkingConfig, ParkedConnection};
pub use outbox::{OutboxBuffer, OutboxDeferralConfig, OutboxError, OutboxFrame};

pub use dns::{DnsOps, DnsQuery, DnsRcode, DnsResponse, RESOLV_CONF_PATH};
pub use l4::{L4DivertOps, L4Stream};
pub use net::{
    FrameClass, FrameDirection, LinkLocalRoute, NetOps, WireWiring, classify_frame,
    classify_frame_directional, dns_payload, is_synthetic_dns, link_local_route,
};
pub use udp_nat::{UDP_NAT_IDLE_TIMEOUT_SECS, UDP_NAT_SLAB_CAPACITY, UdpNatEntry, UdpNatOps};
