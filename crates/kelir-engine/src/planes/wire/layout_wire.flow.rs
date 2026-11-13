// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#Userspace-network-stack

pub(crate) use kelir_interface::frame_layout_views::{
    ETH_HEADER_BYTES, ETHERTYPE_IPV4, EthView, IP_PROTO_TCP, IPV4_MIN_HEADER_BYTES, Ipv4View,
};

pub(crate) const UDP_HEADER_BYTES: usize = 8;
pub(crate) const L4_PORT_PAIR_BYTES: usize = 4;
pub(crate) const IP_PROTO_UDP: u8 = 17;

mod udp {
    pub(super) const SRC_PORT: usize = 0;
    pub(super) const DST_PORT: usize = 2;
}

// see: cans/07-wire-plane.md#Userspace-network-stack
pub(crate) struct UdpView<'a> {
    bytes: &'a [u8],
}

impl<'a> UdpView<'a> {
    pub(crate) fn new_unchecked(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    pub(crate) fn src_port(&self) -> u16 {
        u16::from_be_bytes([self.bytes[udp::SRC_PORT], self.bytes[udp::SRC_PORT + 1]])
    }

    pub(crate) fn dst_port(&self) -> u16 {
        u16::from_be_bytes([self.bytes[udp::DST_PORT], self.bytes[udp::DST_PORT + 1]])
    }
}
