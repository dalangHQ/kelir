// see: cans/plugins/wire-ingress.md
#![forbid(unsafe_code)]

// TODO (L3): tests/test_parking.rs per cans/artifacts/config/project-structure.txt.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use kelir_interface::plugin::PluginError;
use kelir_interface::plugin::{
    HandlerCtx, KelirPlugin, LifecycleParticipant, MountScope, PipelineBinder, StreamInterposer,
};
use kelir_interface::protocol::{Channel, StreamDirection, StreamKind, Verdict};

// see: cans/plugins/wire-ingress.md
pub const MAX_PARKED_BYTES: usize = 64 * 1024;

// see: cans/plugins/wire-ingress.md
pub const L4_FORWARD_HEADER_LEN: usize = 14;

// see: cans/plugins/wire-ingress.md
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListenConfig {
    pub port: u16,
    pub protocol: String,
}

// see: cans/plugins/wire-ingress.md
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParkingConfig {
    pub max_buffered_bytes: usize,
    pub idle_timeout_ms: u64,
}

// see: cans/plugins/wire-ingress.md
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WakeOnPacketConfig {
    pub transport_channel: u16,
}

// see: cans/plugins/wire-ingress.md
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BacklogOverflow {
    Reject503,
}

// see: cans/plugins/wire-ingress.md
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BacklogConfig {
    pub max_connections: usize,
    pub overflow: BacklogOverflow,
}

// see: cans/plugins/wire-ingress.md
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireIngressConfig {
    pub listen: ListenConfig,
    pub parking: ParkingConfig,
    pub wake_on_packet: WakeOnPacketConfig,
    pub backlog: BacklogConfig,
}

impl Default for WireIngressConfig {
    fn default() -> Self {
        Self {
            listen: ListenConfig {
                port: 8443,
                protocol: "https".into(),
            },
            parking: ParkingConfig {
                max_buffered_bytes: MAX_PARKED_BYTES,
                idle_timeout_ms: 30_000,
            },
            wake_on_packet: WakeOnPacketConfig {
                transport_channel: 0x0001,
            },
            backlog: BacklogConfig {
                max_connections: 256,
                overflow: BacklogOverflow::Reject503,
            },
        }
    }
}

// see: cans/plugins/wire-ingress.md
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct L4ForwardFrame {
    pub target_port: u16,
    pub connection_id: u64,
    pub payload_len: u32,
}

impl L4ForwardFrame {
    pub fn decode(payload: &[u8]) -> Option<(Self, &[u8])> {
        if payload.len() < L4_FORWARD_HEADER_LEN {
            return None;
        }
        let target_port = u16::from_le_bytes([payload[0], payload[1]]);
        let connection_bytes: [u8; 8] = payload[2..10].try_into().ok()?;
        let connection_id = u64::from_le_bytes(connection_bytes);
        let len_bytes: [u8; 4] = payload[10..14].try_into().ok()?;
        let payload_len = u32::from_le_bytes(len_bytes);
        if payload.len() - L4_FORWARD_HEADER_LEN != payload_len as usize {
            return None;
        }
        let frame = Self {
            target_port,
            connection_id,
            payload_len,
        };
        Some((frame, &payload[L4_FORWARD_HEADER_LEN..]))
    }
}

// see: cans/plugins/wire-ingress.md
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParkOutcome {
    Parked,
    Overflow,
}

// see: cans/plugins/wire-ingress.md
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParkedConnection {
    target_port: u16,
    buffered: Vec<u8>,
}

// see: cans/plugins/wire-ingress.md
// TODO (L3): idle-timeout eviction, suspend trigger, and TCP RST on rollback need the driver suspend hook and a timer source.
#[derive(Debug, Default)]
struct IngressState {
    parked: Mutex<HashMap<u64, ParkedConnection>>,
    streams: Mutex<HashMap<u32, u64>>,
    pending_wake: Mutex<bool>,
}

impl IngressState {
    fn park(
        &self,
        stream_id: u32,
        frame: L4ForwardFrame,
        initial: &[u8],
        config: &WireIngressConfig,
    ) -> Result<ParkOutcome, PluginError> {
        {
            let mut parked = self
                .parked
                .lock()
                .map_err(|_| PluginError::Halt("wire-ingress parked table poisoned".into()))?;
            match parked.get_mut(&frame.connection_id) {
                Some(existing) => {
                    if existing.target_port != frame.target_port {
                        return Err(PluginError::reject(
                            "EPROTO",
                            "L4_FORWARD connection id reused for a different target port",
                        ));
                    }
                    if existing.buffered.len() + initial.len() > config.parking.max_buffered_bytes {
                        return Ok(ParkOutcome::Overflow);
                    }
                    existing.buffered.extend_from_slice(initial);
                }
                None => {
                    if parked.len() >= config.backlog.max_connections {
                        return Ok(ParkOutcome::Overflow);
                    }
                    if initial.len() > config.parking.max_buffered_bytes {
                        return Ok(ParkOutcome::Overflow);
                    }
                    parked.insert(
                        frame.connection_id,
                        ParkedConnection {
                            target_port: frame.target_port,
                            buffered: initial.to_vec(),
                        },
                    );
                    let mut pending_wake = self
                        .pending_wake
                        .lock()
                        .map_err(|_| PluginError::Halt("wire-ingress wake flag poisoned".into()))?;
                    *pending_wake = true;
                }
            }
        }
        let mut streams = self
            .streams
            .lock()
            .map_err(|_| PluginError::Halt("wire-ingress stream table poisoned".into()))?;
        streams.insert(stream_id, frame.connection_id);
        Ok(ParkOutcome::Parked)
    }

    fn release(&self, stream_id: u32) -> Result<(), PluginError> {
        let connection_id = {
            let mut streams = self
                .streams
                .lock()
                .map_err(|_| PluginError::Halt("wire-ingress stream table poisoned".into()))?;
            streams.remove(&stream_id)
        };
        if let Some(connection_id) = connection_id {
            let mut parked = self
                .parked
                .lock()
                .map_err(|_| PluginError::Halt("wire-ingress parked table poisoned".into()))?;
            parked.remove(&connection_id);
        }
        Ok(())
    }

    // see: cans/plugins/wire-ingress.md
    fn reset(&self) -> Result<Vec<u64>, PluginError> {
        let mut parked = self
            .parked
            .lock()
            .map_err(|_| PluginError::Halt("wire-ingress parked table poisoned".into()))?;
        let connection_ids: Vec<u64> = parked.keys().copied().collect();
        parked.clear();
        drop(parked);
        let mut streams = self
            .streams
            .lock()
            .map_err(|_| PluginError::Halt("wire-ingress stream table poisoned".into()))?;
        streams.clear();
        Ok(connection_ids)
    }

    // see: cans/plugins/wire-ingress.md
    fn take_pending_wake(&self) -> Result<bool, PluginError> {
        let mut pending_wake = self
            .pending_wake
            .lock()
            .map_err(|_| PluginError::Halt("wire-ingress wake flag poisoned".into()))?;
        let requested = *pending_wake;
        *pending_wake = false;
        Ok(requested)
    }
}

// see: cans/plugins/wire-ingress.md
#[derive(Clone)]
pub struct IngressRoutingService {
    state: Arc<IngressState>,
    config: WireIngressConfig,
}

impl IngressRoutingService {
    pub fn park_connection(
        &self,
        stream_id: u32,
        frame: L4ForwardFrame,
        initial: &[u8],
    ) -> Result<ParkOutcome, PluginError> {
        self.state.park(stream_id, frame, initial, &self.config)
    }

    pub fn release_connection(&self, stream_id: u32) -> Result<(), PluginError> {
        self.state.release(stream_id)
    }

    pub fn reset_parked_connections(&self) -> Result<Vec<u64>, PluginError> {
        self.state.reset()
    }

    // see: cans/plugins/wire-ingress.md
    // TODO (L3): host loop polls take_pending_wake and resumes via SessionWakeHandle::resume; no such handle exists in-tree and VmHandle::resume is still a stub.
    pub fn take_pending_wake(&self) -> Result<bool, PluginError> {
        self.state.take_pending_wake()
    }
}

impl kelir_interface::plugin::ParkedConnectionReset for IngressRoutingService {
    fn reset_parked_connections(&self) -> Result<Vec<u64>, PluginError> {
        self.state.reset()
    }
}

// see: cans/plugins/wire-ingress.md
// TODO (L3): host TCP accept/termination and suspend parking I/O per cans/plugins/wire-ingress.md.
#[derive(Clone)]
pub struct WireIngressPlugin {
    pub router: IngressRoutingService,
    config: WireIngressConfig,
}

impl WireIngressPlugin {
    pub fn new() -> Self {
        Self::with_config(WireIngressConfig::default())
    }

    pub fn with_config(config: WireIngressConfig) -> Self {
        let state = Arc::new(IngressState::default());
        Self {
            router: IngressRoutingService {
                state,
                config: config.clone(),
            },
            config,
        }
    }

    pub fn wake_channel(&self) -> Option<Channel> {
        Channel::from_u16(self.config.wake_on_packet.transport_channel)
    }

    pub fn reset_parked_connections(&self) -> Result<Vec<u64>, PluginError> {
        self.router.reset_parked_connections()
    }

    // see: cans/plugins/wire-ingress.md
    pub fn take_pending_wake(&self) -> Result<bool, PluginError> {
        self.router.take_pending_wake()
    }
}

impl Default for WireIngressPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for WireIngressPlugin {
    type Context = ();
    fn context(&self) {}
    fn name(&self) -> &'static str {
        "wire-ingress"
    }
    fn default_scope(&self) -> MountScope {
        MountScope::default()
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        let stage: Arc<dyn StreamInterposer> = Arc::new(self.clone());
        pipeline.bind_stream_scoped(StreamKind::L4Forward, stage, self.default_scope());
        pipeline
            .service_registry()
            .register(Arc::new(self.router.clone()));
        pipeline
            .service_registry()
            .register(Arc::new(self.router.clone())
                as Arc<dyn kelir_interface::plugin::ParkedConnectionReset>);
    }
}

impl LifecycleParticipant for WireIngressPlugin {}

#[async_trait]
impl StreamInterposer for WireIngressPlugin {
    async fn on_stream_open(
        &self,
        stream_id: u32,
        tags: &mut Vec<u8>,
        ctx: &HandlerCtx,
    ) -> Result<Verdict, PluginError> {
        let _ = (stream_id, tags, ctx);
        // TODO (L3): validate L4_FORWARD open tags per cans/11-protocol.md#Stream-mux-framing.
        Ok(Verdict::Pass)
    }

    async fn on_stream_data(
        &self,
        direction: StreamDirection,
        stream_id: u32,
        payload: &mut Vec<u8>,
        ctx: &HandlerCtx,
    ) -> Result<Verdict, PluginError> {
        let _ = ctx;
        match direction {
            StreamDirection::GuestToHost => {
                let Some((frame, initial)) = L4ForwardFrame::decode(payload) else {
                    return Ok(Verdict::Drop {
                        errno: None,
                        reason: "L4_FORWARD frame header malformed or payload_len mismatched"
                            .into(),
                    });
                };
                match self.router.park_connection(stream_id, frame, initial)? {
                    ParkOutcome::Parked => Ok(Verdict::Pass),
                    ParkOutcome::Overflow => Ok(Verdict::Drop {
                        errno: None,
                        reason: "wire-ingress backlog overflow: 503 + Retry-After: 1".into(),
                    }),
                }
            }
            StreamDirection::HostToGuest => {
                // TODO (L3): splice parked bytes to 127.0.0.1:target_port via the guest shim ingress forwarder per cans/plugins/wire-ingress.md.
                let _ = payload;
                Ok(Verdict::Pass)
            }
        }
    }

    async fn on_stream_close(&self, stream_id: u32, ctx: &HandlerCtx) -> Result<(), PluginError> {
        let _ = ctx;
        self.router.release_connection(stream_id)
    }
}
