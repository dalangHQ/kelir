// see: cans/plugins/wire-ingress.md#Plugin-wire-ingress
#![forbid(unsafe_code)]

// TODO (L3): tests/test_parking.rs per cans/artifacts/config/project-structure.txt.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use kelir_interface::driver_interface::{ExternalSuspendKind, SuspendReason};
use kelir_interface::handles_lifecycle::VmHandle;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{
    KelirPlugin, LifecycleParticipant, MountScope, NetInterposer, ParkedConnectionReset,
    PipelineBinder, VirtioDevice,
};
use kelir_interface::protocol_wire::{Channel, FilterAction, SuspendUntil};

// see: cans/plugins/wire-ingress.md#Host-proxy-terminates-TCP-handshake-and-buffers-up-to-64KB-initial-request-data
pub const MAX_PARKED_BYTES: usize = 64 * 1024;

// see: cans/plugins/wire-ingress.md#Ingress-stream-framing-on-stream-kind-L4-FORWARD-(mux-0x0001):
pub const L4_FORWARD_HEADER_LEN: usize = 14;

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

// see: cans/plugins/wire-ingress.md#Ingress-stream-framing-on-stream-kind-L4-FORWARD-(mux-0x0001):
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

// see: cans/plugins/wire-ingress.md#Connection-parking-on-suspend
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParkOutcome {
    Parked,
    Overflow,
}

// see: cans/plugins/wire-ingress.md#Connection-parking-on-suspend
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParkedConnection {
    target_port: u16,
    buffered: Vec<u8>,
}

// see: cans/plugins/wire-ingress.md#Wakes-target-microVM-via-SessionWakeHandle::resume
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParkedDrain {
    pub connection_id: u64,
    pub target_port: u16,
    pub buffered: Vec<u8>,
}

// see: cans/plugins/wire-ingress.md#Connection-parking-on-suspend
#[derive(Debug, Default)]
struct IngressState {
    parked: Mutex<HashMap<u64, ParkedConnection>>,
    pending_wake: Mutex<bool>,
    suspended: Mutex<bool>,
    suspend_path: Mutex<Option<String>>,
}

impl IngressState {
    fn park(
        &self,
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
                }
            }
        }
        if *self
            .suspended
            .lock()
            .map_err(|_| PluginError::Halt("wire-ingress suspend flag poisoned".into()))?
        {
            let mut pending_wake = self
                .pending_wake
                .lock()
                .map_err(|_| PluginError::Halt("wire-ingress wake flag poisoned".into()))?;
            *pending_wake = true;
        }
        Ok(ParkOutcome::Parked)
    }

    fn is_suspended(&self) -> Result<bool, PluginError> {
        self.suspended
            .lock()
            .map_err(|_| PluginError::Halt("wire-ingress suspend flag poisoned".into()))
            .map(|flag| *flag)
    }

    fn begin_parking(&self, path: &str) -> Result<(), PluginError> {
        *self
            .suspended
            .lock()
            .map_err(|_| PluginError::Halt("wire-ingress suspend flag poisoned".into()))? = true;
        *self
            .suspend_path
            .lock()
            .map_err(|_| PluginError::Halt("wire-ingress suspend path poisoned".into()))? =
            Some(path.to_string());
        Ok(())
    }

    fn take_pending_wake(&self) -> Result<bool, PluginError> {
        let mut pending_wake = self
            .pending_wake
            .lock()
            .map_err(|_| PluginError::Halt("wire-ingress wake flag poisoned".into()))?;
        let requested = *pending_wake;
        *pending_wake = false;
        Ok(requested)
    }

    fn take_suspend_path(&self) -> Result<String, PluginError> {
        self.suspend_path
            .lock()
            .map_err(|_| PluginError::Halt("wire-ingress suspend path poisoned".into()))
            .and_then(|mut path| {
                path.take().ok_or_else(|| {
                    PluginError::Halt("wire-ingress wake without a parking suspend".into())
                })
            })
    }

    fn drain_parked(&self) -> Result<Vec<ParkedDrain>, PluginError> {
        let mut parked = self
            .parked
            .lock()
            .map_err(|_| PluginError::Halt("wire-ingress parked table poisoned".into()))?;
        let drained = parked
            .drain()
            .map(|(connection_id, connection)| ParkedDrain {
                connection_id,
                target_port: connection.target_port,
                buffered: connection.buffered,
            })
            .collect();
        *self
            .suspended
            .lock()
            .map_err(|_| PluginError::Halt("wire-ingress suspend flag poisoned".into()))? = false;
        Ok(drained)
    }

    fn release(&self, connection_id: u64) -> Result<(), PluginError> {
        let mut parked = self
            .parked
            .lock()
            .map_err(|_| PluginError::Halt("wire-ingress parked table poisoned".into()))?;
        parked.remove(&connection_id);
        Ok(())
    }

    // see: cans/plugins/wire-ingress.md#Connection-parking-on-suspend
    fn reset(&self) -> Result<Vec<u64>, PluginError> {
        let mut parked = self
            .parked
            .lock()
            .map_err(|_| PluginError::Halt("wire-ingress parked table poisoned".into()))?;
        let connection_ids: Vec<u64> = parked.keys().copied().collect();
        parked.clear();
        Ok(connection_ids)
    }
}

// see: cans/plugins/wire-ingress.md#Service-exports-(wire-ingress)
#[derive(Clone)]
pub struct IngressRoutingService {
    state: Arc<IngressState>,
    config: WireIngressConfig,
}

impl IngressRoutingService {
    pub fn park_connection(
        &self,
        frame: L4ForwardFrame,
        initial: &[u8],
    ) -> Result<ParkOutcome, PluginError> {
        self.state.park(frame, initial, &self.config)
    }

    pub fn release_connection(&self, connection_id: u64) -> Result<(), PluginError> {
        self.state.release(connection_id)
    }

    pub fn reset_parked_connections(&self) -> Result<Vec<u64>, PluginError> {
        self.state.reset()
    }

    pub fn is_suspended(&self) -> Result<bool, PluginError> {
        self.state.is_suspended()
    }

    // see: cans/plugins/wire-ingress.md#Connection-parking-on-suspend
    pub async fn suspend_for_parking(&self, vm: &VmHandle, path: &str) -> Result<(), PluginError> {
        vm.suspend(
            SuspendReason::External(ExternalSuspendKind::WireConnectionParking),
            path,
        )
        .await?;
        self.state.begin_parking(path)
    }

    // see: cans/plugins/wire-ingress.md#Wakes-target-microVM-via-SessionWakeHandle::resume
    pub async fn resume_parked(&self, vm: &VmHandle) -> Result<Vec<ParkedDrain>, PluginError> {
        if !self.state.take_pending_wake()? {
            return Ok(Vec::new());
        }
        let path = self.state.take_suspend_path()?;
        vm.resume(&path).await?;
        self.state.drain_parked()
    }
}

impl ParkedConnectionReset for IngressRoutingService {
    fn reset_parked_connections(&self) -> Result<Vec<u64>, PluginError> {
        self.state.reset()
    }
}

// see: cans/plugins/wire-ingress.md#Plugin-wire-ingress
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

    // see: cans/plugins/wire-ingress.md#Wakes-target-microVM-via-SessionWakeHandle::resume
    pub async fn suspend_for_parking(&self, vm: &VmHandle, path: &str) -> Result<(), PluginError> {
        self.router.suspend_for_parking(vm, path).await
    }

    // see: cans/plugins/wire-ingress.md#Wakes-target-microVM-via-SessionWakeHandle::resume
    pub async fn resume_parked(&self, vm: &VmHandle) -> Result<Vec<ParkedDrain>, PluginError> {
        self.router.resume_parked(vm).await
    }
}

impl Default for WireIngressPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KelirPlugin for WireIngressPlugin {
    type Context = IngressRoutingService;
    fn context(&self) -> IngressRoutingService {
        self.router.clone()
    }
    fn name(&self) -> &'static str {
        "wire-ingress"
    }
    fn default_scope(&self) -> MountScope {
        // see: cans/11-protocol.md#Control-channel
        MountScope::default().with_devices([VirtioDevice::Net, VirtioDevice::Vsock])
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        // see: cans/07-wire-plane.md#Raw-packet-interception-points
        let net_stage: Arc<dyn NetInterposer> = Arc::new(self.clone());
        pipeline.bind_net_scoped(net_stage, self.default_scope());
        // see: cans/plugins/wire-ingress.md#Connection-parking-on-suspend
        pipeline.bind_parked_connection_reset(Arc::new(self.router.clone()));
    }
}

impl LifecycleParticipant for WireIngressPlugin {}

// see: cans/plugins/wire-ingress.md#Ingress-stream-framing-on-stream-kind-L4-FORWARD-(mux-0x0001):
impl NetInterposer for WireIngressPlugin {
    fn on_net_inbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        // TODO (L3): L2/L4 reassembly per cans/07-wire-plane.md before L4_FORWARD payloads reach this ring.
        if !self.router.is_suspended()? {
            return Ok(FilterAction::Forward);
        }
        let Some((l4, initial)) = L4ForwardFrame::decode(frame) else {
            return Ok(FilterAction::Forward);
        };
        match self.router.park_connection(l4, initial)? {
            ParkOutcome::Parked => Ok(FilterAction::Suspend {
                until: SuspendUntil::Deadline(self.config.parking.idle_timeout_ms),
            }),
            ParkOutcome::Overflow => Ok(FilterAction::Drop {
                errno: None,
                reason: "wire-ingress backlog overflow: 503 + Retry-After: 1".into(),
            }),
        }
    }
}
