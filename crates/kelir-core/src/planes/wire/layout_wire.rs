// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#Userspace-network-stack

pub(crate) const ETH_HEADER_BYTES: usize = 14;
pub(crate) const ETHERTYPE_IPV4: u16 = 0x0800;
pub(crate) const IPV4_MIN_HEADER_BYTES: usize = 20;
pub(crate) const TCP_MIN_HEADER_BYTES: usize = 20;
pub(crate) const UDP_HEADER_BYTES: usize = 8;
pub(crate) const L4_PORT_PAIR_BYTES: usize = 4;
pub(crate) const IP_PROTO_TCP: u8 = 6;
pub(crate) const IP_PROTO_UDP: u8 = 17;

pub(crate) mod eth {
    pub(crate) const ETHERTYPE: usize = 12;
}

pub(crate) mod ipv4 {
    pub(crate) const VERSION_IHL: usize = 0;
    pub(crate) const PROTO: usize = 9;
    pub(crate) const SRC: usize = 12;
    pub(crate) const DST: usize = 16;
}

pub(crate) mod tcp {
    pub(crate) const SRC_PORT: usize = 0;
    pub(crate) const DST_PORT: usize = 2;
    pub(crate) const DATA_OFFSET: usize = 12;
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

    pub(crate) fn ihl_bytes_checked(&self) -> Option<usize> {
        self.bytes
            .get(ipv4::VERSION_IHL)
            .map(|byte| usize::from(byte & 0x0f) * 4)
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

    pub(crate) fn src_checked(&self) -> Option<[u8; 4]> {
        self.bytes
            .get(ipv4::SRC..ipv4::SRC + 4)
            .and_then(|slice| slice.try_into().ok())
    }

    pub(crate) fn dst_checked(&self) -> Option<[u8; 4]> {
        self.bytes
            .get(ipv4::DST..ipv4::DST + 4)
            .and_then(|slice| slice.try_into().ok())
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

    pub(crate) fn header_bytes_checked(&self) -> Option<usize> {
        self.bytes
            .get(tcp::DATA_OFFSET)
            .map(|byte| usize::from(byte >> 4) * 4)
    }
}
