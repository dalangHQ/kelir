// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
// see: cans/07-wire-plane.md#Frame-drop-and-forward-semantics
// see: cans/plugins/db-proxy.md#Invariants-and-policy-(db-proxy)

pub const ETHERTYPE_IPV4: u16 = 0x0800;
pub const IP_PROTO_TCP: u8 = 6;
pub const TCP_FLAG_FIN: u8 = 0x01;
pub const TCP_FLAG_SYN: u8 = 0x02;
pub const TCP_FLAG_RST: u8 = 0x04;

pub(crate) const ETH_HEADER_BYTES: usize = 14;
pub(crate) const IPV4_VERSION: u8 = 4;
pub(crate) const IPV4_MIN_HEADER_WORDS: usize = 5;
pub(crate) const IPV4_MIN_HEADER_BYTES: usize = IPV4_MIN_HEADER_WORDS * 4;
pub(crate) const IPV4_FRAGMENT_OFFSET_MASK: u16 = 0x1fff;
pub(crate) const TCP_MIN_HEADER_WORDS: usize = 5;
pub(crate) const TCP_MIN_HEADER_BYTES: usize = TCP_MIN_HEADER_WORDS * 4;
pub(crate) const SEQ_HALF_WINDOW: u32 = 0x8000_0000;
pub(crate) const PG_HEADER_BYTES: usize = 5;
pub(crate) const PG_TYPE_BYTES: usize = 1;
pub(crate) const PG_MIN_LENGTH: usize = 4;

pub(crate) mod eth {
    pub(crate) const ETHERTYPE: usize = 12;
}

pub(crate) mod ipv4 {
    pub(crate) const VERSION_IHL: usize = 0;
    pub(crate) const TOTAL_LEN: usize = 2;
    pub(crate) const FRAGMENT: usize = 6;
    pub(crate) const PROTO: usize = 9;
    pub(crate) const SRC: usize = 12;
    pub(crate) const DST: usize = 16;
}

pub(crate) mod tcp {
    pub(crate) const SRC_PORT: usize = 0;
    pub(crate) const DST_PORT: usize = 2;
    pub(crate) const SEQ: usize = 4;
    pub(crate) const DATA_OFFSET: usize = 12;
    pub(crate) const FLAGS: usize = 13;
}

pub(crate) mod pg {
    pub(crate) const MSG_TYPE: usize = 0;
    pub(crate) const LENGTH: usize = 1;
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct EthView<'a> {
    bytes: &'a [u8],
}

impl<'a> EthView<'a> {
    pub(crate) fn new_unchecked(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    pub(crate) fn etype(&self) -> u16 {
        u16::from_be_bytes([self.bytes[eth::ETHERTYPE], self.bytes[eth::ETHERTYPE + 1]])
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Ipv4View<'a> {
    bytes: &'a [u8],
}

impl<'a> Ipv4View<'a> {
    pub(crate) fn new_unchecked(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    pub(crate) fn version(&self) -> u8 {
        self.bytes[ipv4::VERSION_IHL] >> 4
    }

    pub(crate) fn ihl_words(&self) -> usize {
        usize::from(self.bytes[ipv4::VERSION_IHL] & 0x0f)
    }

    pub(crate) fn ihl_bytes(&self) -> usize {
        self.ihl_words() * 4
    }

    pub(crate) fn total_len(&self) -> u16 {
        u16::from_be_bytes([self.bytes[ipv4::TOTAL_LEN], self.bytes[ipv4::TOTAL_LEN + 1]])
    }

    pub(crate) fn fragment(&self) -> u16 {
        u16::from_be_bytes([self.bytes[ipv4::FRAGMENT], self.bytes[ipv4::FRAGMENT + 1]])
    }

    pub(crate) fn proto(&self) -> u8 {
        self.bytes[ipv4::PROTO]
    }

    pub(crate) fn src(&self) -> [u8; 4] {
        [
            self.bytes[ipv4::SRC],
            self.bytes[ipv4::SRC + 1],
            self.bytes[ipv4::SRC + 2],
            self.bytes[ipv4::SRC + 3],
        ]
    }

    pub(crate) fn dst(&self) -> [u8; 4] {
        [
            self.bytes[ipv4::DST],
            self.bytes[ipv4::DST + 1],
            self.bytes[ipv4::DST + 2],
            self.bytes[ipv4::DST + 3],
        ]
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TcpView<'a> {
    bytes: &'a [u8],
}

impl<'a> TcpView<'a> {
    pub(crate) fn new_unchecked(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    pub(crate) fn src_port(&self) -> u16 {
        u16::from_be_bytes([self.bytes[tcp::SRC_PORT], self.bytes[tcp::SRC_PORT + 1]])
    }

    pub(crate) fn dst_port(&self) -> u16 {
        u16::from_be_bytes([self.bytes[tcp::DST_PORT], self.bytes[tcp::DST_PORT + 1]])
    }

    pub(crate) fn seq(&self) -> u32 {
        u32::from_be_bytes([
            self.bytes[tcp::SEQ],
            self.bytes[tcp::SEQ + 1],
            self.bytes[tcp::SEQ + 2],
            self.bytes[tcp::SEQ + 3],
        ])
    }

    pub(crate) fn flags(&self) -> u8 {
        self.bytes[tcp::FLAGS]
    }

    pub(crate) fn header_bytes(&self) -> usize {
        usize::from(self.bytes[tcp::DATA_OFFSET] >> 4) * 4
    }
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
