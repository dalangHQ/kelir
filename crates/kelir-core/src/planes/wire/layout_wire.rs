// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#Userspace-network-stack

pub(crate) use kelir_interface::frame_layout_views::{
    ETH_HEADER_BYTES, ETHERTYPE_IPV4, EthView, IP_PROTO_TCP, IPV4_MIN_HEADER_BYTES, Ipv4View,
    TCP_MIN_HEADER_BYTES, TcpView,
};

pub(crate) const UDP_HEADER_BYTES: usize = 8;
pub(crate) const L4_PORT_PAIR_BYTES: usize = 4;
pub(crate) const IP_PROTO_UDP: u8 = 17;
