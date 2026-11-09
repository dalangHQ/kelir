// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#Userspace-network-stack

pub const ETH_HEADER_BYTES: usize = 14;
pub const ETHERTYPE_IPV4: u16 = 0x0800;
pub const IP_PROTO_TCP: u8 = 6;
pub const IPV4_MIN_HEADER_WORDS: usize = 5;
pub const IPV4_MIN_HEADER_BYTES: usize = IPV4_MIN_HEADER_WORDS * 4;
pub const TCP_MIN_HEADER_WORDS: usize = 5;
pub const TCP_MIN_HEADER_BYTES: usize = TCP_MIN_HEADER_WORDS * 4;

mod eth {
    pub(super) const ETHERTYPE: usize = 12;
}

mod ipv4 {
    pub(super) const VERSION_IHL: usize = 0;
    pub(super) const TOTAL_LEN: usize = 2;
    pub(super) const FRAGMENT: usize = 6;
    pub(super) const PROTO: usize = 9;
    pub(super) const SRC: usize = 12;
    pub(super) const DST: usize = 16;
}

mod tcp {
    pub(super) const SRC_PORT: usize = 0;
    pub(super) const DST_PORT: usize = 2;
    pub(super) const SEQ: usize = 4;
    pub(super) const DATA_OFFSET: usize = 12;
    pub(super) const FLAGS: usize = 13;
}

#[derive(Debug, Clone, Copy)]
pub struct EthView<'a> {
    bytes: &'a [u8],
}

impl<'a> EthView<'a> {
    pub fn new_unchecked(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    pub fn etype(&self) -> u16 {
        u16::from_be_bytes([self.bytes[eth::ETHERTYPE], self.bytes[eth::ETHERTYPE + 1]])
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Ipv4View<'a> {
    bytes: &'a [u8],
}

impl<'a> Ipv4View<'a> {
    pub fn new_unchecked(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    pub fn version(&self) -> u8 {
        self.bytes[ipv4::VERSION_IHL] >> 4
    }

    pub fn ihl_words(&self) -> usize {
        usize::from(self.bytes[ipv4::VERSION_IHL] & 0x0f)
    }

    pub fn ihl_bytes(&self) -> usize {
        self.ihl_words() * 4
    }

    pub fn ihl_bytes_checked(&self) -> Option<usize> {
        self.bytes
            .get(ipv4::VERSION_IHL)
            .map(|byte| usize::from(byte & 0x0f) * 4)
    }

    pub fn total_len(&self) -> u16 {
        u16::from_be_bytes([self.bytes[ipv4::TOTAL_LEN], self.bytes[ipv4::TOTAL_LEN + 1]])
    }

    pub fn fragment(&self) -> u16 {
        u16::from_be_bytes([self.bytes[ipv4::FRAGMENT], self.bytes[ipv4::FRAGMENT + 1]])
    }

    pub fn proto(&self) -> u8 {
        self.bytes[ipv4::PROTO]
    }

    pub fn src(&self) -> [u8; 4] {
        [
            self.bytes[ipv4::SRC],
            self.bytes[ipv4::SRC + 1],
            self.bytes[ipv4::SRC + 2],
            self.bytes[ipv4::SRC + 3],
        ]
    }

    pub fn dst(&self) -> [u8; 4] {
        [
            self.bytes[ipv4::DST],
            self.bytes[ipv4::DST + 1],
            self.bytes[ipv4::DST + 2],
            self.bytes[ipv4::DST + 3],
        ]
    }

    pub fn src_checked(&self) -> Option<[u8; 4]> {
        self.bytes
            .get(ipv4::SRC..ipv4::SRC + 4)
            .and_then(|slice| slice.try_into().ok())
    }

    pub fn dst_checked(&self) -> Option<[u8; 4]> {
        self.bytes
            .get(ipv4::DST..ipv4::DST + 4)
            .and_then(|slice| slice.try_into().ok())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TcpView<'a> {
    bytes: &'a [u8],
}

impl<'a> TcpView<'a> {
    pub fn new_unchecked(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    pub fn src_port(&self) -> u16 {
        u16::from_be_bytes([self.bytes[tcp::SRC_PORT], self.bytes[tcp::SRC_PORT + 1]])
    }

    pub fn dst_port(&self) -> u16 {
        u16::from_be_bytes([self.bytes[tcp::DST_PORT], self.bytes[tcp::DST_PORT + 1]])
    }

    pub fn seq(&self) -> u32 {
        u32::from_be_bytes([
            self.bytes[tcp::SEQ],
            self.bytes[tcp::SEQ + 1],
            self.bytes[tcp::SEQ + 2],
            self.bytes[tcp::SEQ + 3],
        ])
    }

    pub fn flags(&self) -> u8 {
        self.bytes[tcp::FLAGS]
    }

    pub fn header_bytes(&self) -> usize {
        usize::from(self.bytes[tcp::DATA_OFFSET] >> 4) * 4
    }

    pub fn header_bytes_checked(&self) -> Option<usize> {
        self.bytes
            .get(tcp::DATA_OFFSET)
            .map(|byte| usize::from(byte >> 4) * 4)
    }
}
