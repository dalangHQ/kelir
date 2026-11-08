// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::{Mutex, MutexGuard};

use kelir_interface::codec::MAX_FRAME_PAYLOAD;
use kelir_interface::plugin::PluginError;

use crate::pgwire::PgFrameDescriptor;

// see: cans/07-wire-plane.md#Userspace-network-stack
pub const ETHERTYPE_IPV4: u16 = 0x0800;
// see: cans/07-wire-plane.md#Userspace-network-stack
pub const IP_PROTO_TCP: u8 = 6;
// see: cans/07-wire-plane.md#Frame-drop-and-forward-semantics
pub const TCP_FLAG_FIN: u8 = 0x01;
// see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
pub const TCP_FLAG_SYN: u8 = 0x02;
// see: cans/07-wire-plane.md#Frame-drop-and-forward-semantics
pub const TCP_FLAG_RST: u8 = 0x04;

// see: cans/07-wire-plane.md#Wire-architecture
const ETH_HEADER_BYTES: usize = 14;
// see: cans/07-wire-plane.md#Wire-architecture
const IPV4_MIN_HEADER_WORDS: usize = 5;
// see: cans/07-wire-plane.md#Wire-architecture
const TCP_MIN_HEADER_WORDS: usize = 5;
// see: cans/07-wire-plane.md#Wire-architecture
const IPV4_FRAGMENT_OFFSET_MASK: u16 = 0x1fff;
// see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
const SEQ_HALF_WINDOW: u32 = 0x8000_0000;
// see: cans/plugins/db-proxy.md#Invariants-and-policy-(db-proxy)
const PG_HEADER_BYTES: usize = 5;

// see: cans/07-wire-plane.md#Raw-packet-interception-points
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetDirection {
    GuestToUpstream,
    UpstreamToGuest,
}

// see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FlowKey {
    pub client: SocketAddrV4,
    pub server: SocketAddrV4,
}

// see: cans/07-wire-plane.md#Wire-architecture
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcpSegmentView<'a> {
    pub src: SocketAddrV4,
    pub dst: SocketAddrV4,
    pub seq: u32,
    pub flags: u8,
    pub payload: &'a [u8],
}

// see: cans/07-wire-plane.md#Wire-architecture
// TODO (L3): verify Ethernet, IPv4, and TCP checksums and reject invalid frames.
pub fn parse_tcp_segment(frame: &[u8]) -> Option<TcpSegmentView<'_>> {
    if frame.len() < ETH_HEADER_BYTES + IPV4_MIN_HEADER_WORDS * 4 {
        return None;
    }
    let ethertype = u16::from_be_bytes([frame[12], frame[13]]);
    if ethertype != ETHERTYPE_IPV4 {
        return None;
    }
    let version_ihl = frame[14];
    if version_ihl >> 4 != 4 {
        return None;
    }
    let ihl_words = (version_ihl & 0x0f) as usize;
    if ihl_words < IPV4_MIN_HEADER_WORDS {
        return None;
    }
    let ip_total_len = u16::from_be_bytes([frame[16], frame[17]]) as usize;
    if ip_total_len < (ihl_words + TCP_MIN_HEADER_WORDS) * 4 {
        return None;
    }
    let ip_end = ETH_HEADER_BYTES + ip_total_len;
    if frame.len() < ip_end {
        return None;
    }
    let fragment = u16::from_be_bytes([frame[20], frame[21]]);
    if fragment & IPV4_FRAGMENT_OFFSET_MASK != 0 {
        return None;
    }
    if frame[23] != IP_PROTO_TCP {
        return None;
    }
    let src_ip = Ipv4Addr::new(frame[26], frame[27], frame[28], frame[29]);
    let dst_ip = Ipv4Addr::new(frame[30], frame[31], frame[32], frame[33]);
    let tcp_offset = ETH_HEADER_BYTES + ihl_words * 4;
    if frame.len() < tcp_offset + TCP_MIN_HEADER_WORDS * 4 {
        return None;
    }
    let tcp = &frame[tcp_offset..ip_end];
    let src_port = u16::from_be_bytes([tcp[0], tcp[1]]);
    let dst_port = u16::from_be_bytes([tcp[2], tcp[3]]);
    let seq = u32::from_be_bytes([tcp[4], tcp[5], tcp[6], tcp[7]]);
    let data_offset_words = (tcp[12] >> 4) as usize;
    if data_offset_words < TCP_MIN_HEADER_WORDS {
        return None;
    }
    let header_bytes = data_offset_words * 4;
    if tcp.len() < header_bytes {
        return None;
    }
    Some(TcpSegmentView {
        src: SocketAddrV4::new(src_ip, src_port),
        dst: SocketAddrV4::new(dst_ip, dst_port),
        seq,
        flags: tcp[13],
        payload: &tcp[header_bytes..],
    })
}

// see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
fn seq_after(seq: u32, reference: u32) -> bool {
    let delta = seq.wrapping_sub(reference);
    delta > 0 && delta < SEQ_HALF_WINDOW
}

// see: cans/07-wire-plane.md#Frame-drop-and-forward-semantics
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SegmentOutcome {
    Forwarded,
    Dropped,
    Stream(Vec<u8>),
}

// see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
#[derive(Default)]
pub struct TcpStreamReassembler {
    next_seq: Option<u32>,
    carry: Vec<(u32, Vec<u8>)>,
}

impl TcpStreamReassembler {
    pub fn new() -> Self {
        Self::default()
    }

    fn carry_bytes(&self) -> usize {
        self.carry.iter().map(|(_, bytes)| bytes.len()).sum()
    }

    // see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
    pub fn push_segment(
        &mut self,
        segment: &TcpSegmentView<'_>,
    ) -> Result<SegmentOutcome, PluginError> {
        if segment.flags & TCP_FLAG_SYN != 0 {
            // TODO (L3): full TCP handshake state machine; payload-carrying SYNs (TFO) stay L3.
            self.next_seq = Some(segment.seq.wrapping_add(1));
            return Ok(SegmentOutcome::Forwarded);
        }
        let payload = segment.payload;
        if payload.is_empty() {
            return Ok(SegmentOutcome::Forwarded);
        }
        let Some(expected) = self.next_seq else {
            self.next_seq = Some(segment.seq.wrapping_add(payload.len() as u32));
            return Ok(SegmentOutcome::Stream(payload.to_vec()));
        };
        if segment.seq == expected {
            let mut next = expected.wrapping_add(payload.len() as u32);
            let mut bytes = payload.to_vec();
            while let Some((carry_seq, carry_bytes)) = self.carry.first() {
                if *carry_seq != next {
                    break;
                }
                next = next.wrapping_add(carry_bytes.len() as u32);
                bytes.extend_from_slice(carry_bytes);
                self.carry.remove(0);
            }
            self.next_seq = Some(next);
            return Ok(SegmentOutcome::Stream(bytes));
        }
        if seq_after(segment.seq, expected) {
            // TODO (L3): bounded out-of-order window with overlap trimming per RFC 793.
            if self.carry_bytes() + payload.len() > MAX_FRAME_PAYLOAD {
                return Ok(SegmentOutcome::Dropped);
            }
            self.carry.push((segment.seq, payload.to_vec()));
            self.carry.sort_by_key(|(carry_seq, _)| *carry_seq);
            return Ok(SegmentOutcome::Forwarded);
        }
        // TODO (L3): retransmit and partial-overlap deduplication.
        Ok(SegmentOutcome::Forwarded)
    }
}

// see: cans/plugins/db-proxy.md#Invariants-and-policy-(db-proxy)
#[derive(Default)]
pub struct PgStreamDecoder {
    carryover: Vec<u8>,
}

impl PgStreamDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    // see: cans/plugins/db-proxy.md#Invariants-and-policy-(db-proxy)
    // TODO (L3): the wire plane owns reassembly at L3; this decoder is the in-crate L2 stand-in per cans/07-wire-plane.md#Transparent-L4-stream-diversion.
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<PgFrameDescriptor>, PluginError> {
        self.carryover.extend_from_slice(bytes);
        let mut frames = Vec::new();
        loop {
            if self.carryover.len() < PG_HEADER_BYTES {
                break;
            }
            let length = u32::from_be_bytes([
                self.carryover[1],
                self.carryover[2],
                self.carryover[3],
                self.carryover[4],
            ]) as usize;
            let total = 1 + length;
            if length < 4 || total > MAX_FRAME_PAYLOAD {
                return Err(PluginError::reject(
                    "EPROTO",
                    "pgwire frame length exceeds the 64KB frame budget",
                ));
            }
            if self.carryover.len() < total {
                break;
            }
            frames.push(PgFrameDescriptor::classify(
                self.carryover[0],
                length as u32,
            ));
            self.carryover.drain(..total);
        }
        if self.carryover.len() > MAX_FRAME_PAYLOAD {
            return Err(PluginError::reject(
                "EPROTO",
                "pgwire carryover window exceeds the 64KB frame budget",
            ));
        }
        Ok(frames)
    }
}

// see: cans/plugins/db-proxy.md#L7-pgwire-rewrite-and-2PC-commit
#[derive(Default)]
pub struct PgFlowState {
    pub reassembly: TcpStreamReassembler,
    pub decoder: PgStreamDecoder,
}

impl PgFlowState {
    pub fn new() -> Self {
        Self::default()
    }
}

// see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
#[derive(Default)]
pub struct NetPlaneState {
    flows: Mutex<HashMap<FlowKey, PgFlowState>>,
}

impl NetPlaneState {
    pub fn new() -> Self {
        Self::default()
    }

    // see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
    pub fn lock_flows(&self) -> Result<MutexGuard<'_, HashMap<FlowKey, PgFlowState>>, PluginError> {
        self.flows
            .lock()
            .map_err(|_| PluginError::Halt("db-proxy net flow table poisoned".into()))
    }
}
