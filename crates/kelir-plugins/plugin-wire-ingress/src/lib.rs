// see: cans/plugins/wire-ingress.md#Plugin-wire-ingress
#![forbid(unsafe_code)]

use kelir_interface::protocol_wire::StreamKind;

// TODO (L3) [wire_plane]: tests/test_parking.rs per cans/artifacts/config/project-structure.txt; slop: test_parking.rs missing, park/suspend path untested
#[path = "ingress_state.flow.rs"]
pub mod ingress_state;

pub use ingress_state::{IngressRoutingService, WireIngressPlugin};

// see: cans/plugins/wire-ingress.md#Host-proxy-terminates-TCP-handshake-and-buffers-up-to-64KB-initial-request-data
pub const MAX_PARKED_BYTES: usize = 64 * 1024;

// see: cans/plugins/wire-ingress.md#Ingress-stream-framing-on-stream-kind-L4-FORWARD-(mux-0x0001):
pub const L4_FORWARD_HEADER_LEN: usize = 14;

// see: cans/plugins/wire-ingress.md#Ingress-stream-framing-on-stream-kind-L4-FORWARD-(mux-0x0001):
pub const L4_FORWARD_STREAM_KIND: StreamKind = StreamKind::L4Forward;

// see: cans/plugins/wire-ingress.md#Invariants-and-policy-(wire-ingress)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListenConfig {
    pub port: u16,
    pub protocol: String,
}

// see: cans/plugins/wire-ingress.md#Connection-parking-on-suspend
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParkingConfig {
    pub max_buffered_bytes: usize,
    pub idle_timeout_ms: u64,
}

// see: cans/plugins/wire-ingress.md#Wakes-target-microVM-via-SessionWakeHandle::resume
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WakeOnPacketConfig {
    pub transport_channel: u16,
}

// see: cans/plugins/wire-ingress.md#Return-errno-and-rejection-behavior-(wire-ingress)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BacklogOverflow {
    Reject503,
}

// see: cans/plugins/wire-ingress.md#Return-errno-and-rejection-behavior-(wire-ingress)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BacklogConfig {
    pub max_connections: usize,
    pub overflow: BacklogOverflow,
}

// see: cans/plugins/wire-ingress.md#Invariants-and-policy-(wire-ingress)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireIngressConfig {
    pub listen: ListenConfig,
    pub parking: ParkingConfig,
    pub wake_on_packet: WakeOnPacketConfig,
    pub backlog: BacklogConfig,
}

// see: cans/plugins/wire-ingress.md#Ingress-stream-framing-on-stream-kind-L4-FORWARD-(mux-0x0001):
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct L4ForwardFrame {
    pub target_port: u16,
    pub connection_id: u64,
    pub payload_len: u32,
}

// see: cans/plugins/wire-ingress.md#Connection-parking-on-suspend
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParkOutcome {
    Parked,
    Overflow,
}

// see: cans/plugins/wire-ingress.md#Wakes-target-microVM-via-SessionWakeHandle::resume
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParkedDrain {
    pub connection_id: u64,
    pub target_port: u16,
    pub buffered: Vec<u8>,
}
