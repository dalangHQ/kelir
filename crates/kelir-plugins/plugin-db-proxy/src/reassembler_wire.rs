// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::{Mutex, MutexGuard};

use kelir_interface::codec_protocol::MAX_FRAME_PAYLOAD;
use kelir_interface::plugin_model::PluginError;

use crate::pgwire_db_proxy::PgFrameDescriptor;

mod layout_wire;

pub use self::layout_wire::{
    ETHERTYPE_IPV4, IP_PROTO_TCP, TCP_FLAG_FIN, TCP_FLAG_RST, TCP_FLAG_SYN,
};

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
    if frame.len() < layout_wire::ETH_HEADER_BYTES + layout_wire::IPV4_MIN_HEADER_BYTES {
        return None;
    }
    if layout_wire::EthView::new_unchecked(frame).etype() != layout_wire::ETHERTYPE_IPV4 {
        return None;
    }
    let ip = layout_wire::Ipv4View::new_unchecked(&frame[layout_wire::ETH_HEADER_BYTES..]);
    if ip.version() != layout_wire::IPV4_VERSION {
        return None;
    }
    if ip.ihl_words() < layout_wire::IPV4_MIN_HEADER_WORDS {
        return None;
    }
    let ip_total_len = usize::from(ip.total_len());
    if ip_total_len < ip.ihl_bytes() + layout_wire::TCP_MIN_HEADER_BYTES {
        return None;
    }
    let ip_end = layout_wire::ETH_HEADER_BYTES + ip_total_len;
    if frame.len() < ip_end {
        return None;
    }
    if ip.fragment() & layout_wire::IPV4_FRAGMENT_OFFSET_MASK != 0 {
        return None;
    }
    if ip.proto() != layout_wire::IP_PROTO_TCP {
        return None;
    }
    let src_ip = Ipv4Addr::from(ip.src());
    let dst_ip = Ipv4Addr::from(ip.dst());
    let tcp_offset = layout_wire::ETH_HEADER_BYTES + ip.ihl_bytes();
    if frame.len() < tcp_offset + layout_wire::TCP_MIN_HEADER_BYTES {
        return None;
    }
    let tcp_bytes = &frame[tcp_offset..ip_end];
    let tcp = layout_wire::TcpView::new_unchecked(tcp_bytes);
    let header_bytes = tcp.header_bytes();
    if header_bytes < layout_wire::TCP_MIN_HEADER_BYTES {
        return None;
    }
    if tcp_bytes.len() < header_bytes {
        return None;
    }
    Some(TcpSegmentView {
        src: SocketAddrV4::new(src_ip, tcp.src_port()),
        dst: SocketAddrV4::new(dst_ip, tcp.dst_port()),
        seq: tcp.seq(),
        flags: tcp.flags(),
        payload: &tcp_bytes[header_bytes..],
    })
}

// see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
fn seq_after(seq: u32, reference: u32) -> bool {
    let delta = seq.wrapping_sub(reference);
    delta > 0 && delta < layout_wire::SEQ_HALF_WINDOW
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
    // TODO (L3): converge on WirePlane::divert_l4 once L4 diversion lands.
    pub fn push_segment(
        &mut self,
        segment: &TcpSegmentView<'_>,
    ) -> Result<SegmentOutcome, PluginError> {
        if segment.flags & layout_wire::TCP_FLAG_SYN != 0 {
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
            if self.carryover.len() < layout_wire::PG_HEADER_BYTES {
                break;
            }
            let header = layout_wire::PgHeaderView::new_unchecked(&self.carryover);
            let length = header.length() as usize;
            let total = layout_wire::PG_TYPE_BYTES + length;
            if length < layout_wire::PG_MIN_LENGTH || total > MAX_FRAME_PAYLOAD {
                return Err(PluginError::reject(
                    "EPROTO",
                    "pgwire frame length exceeds the 64KB frame budget",
                ));
            }
            if self.carryover.len() < total {
                break;
            }
            frames.push(PgFrameDescriptor::classify(
                header.msg_type(),
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
