// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
// see: cans/07-wire-plane.md#Frame-drop-and-forward-semantics
// see: cans/plugins/db-proxy.md#Invariants-and-policy-(db-proxy)

pub use kelir_interface::frame_layout_views::{
    ETH_HEADER_BYTES, ETHERTYPE_IPV4, EthView, IP_PROTO_TCP, IPV4_MIN_HEADER_BYTES,
    IPV4_MIN_HEADER_WORDS, Ipv4View, TCP_MIN_HEADER_BYTES, TcpView,
};

pub const TCP_FLAG_FIN: u8 = 0x01;
pub const TCP_FLAG_SYN: u8 = 0x02;
pub const TCP_FLAG_RST: u8 = 0x04;

pub(crate) const IPV4_VERSION: u8 = 4;
pub(crate) const IPV4_FRAGMENT_OFFSET_MASK: u16 = 0x1fff;
pub(crate) const SEQ_HALF_WINDOW: u32 = 0x8000_0000;
pub(crate) const PG_HEADER_BYTES: usize = 5;
pub(crate) const PG_TYPE_BYTES: usize = 1;
pub(crate) const PG_MIN_LENGTH: usize = 4;

pub(crate) mod pg {
    pub(crate) const MSG_TYPE: usize = 0;
    pub(crate) const LENGTH: usize = 1;
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PgHeaderView<'a> {
    bytes: &'a [u8],
}

impl<'a> PgHeaderView<'a> {
    pub(crate) fn new_unchecked(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    pub(crate) fn msg_type(&self) -> u8 {
        self.bytes[pg::MSG_TYPE]
    }

    pub(crate) fn length(&self) -> u32 {
        u32::from_be_bytes([
            self.bytes[pg::LENGTH],
            self.bytes[pg::LENGTH + 1],
            self.bytes[pg::LENGTH + 2],
            self.bytes[pg::LENGTH + 3],
        ])
    }
}
