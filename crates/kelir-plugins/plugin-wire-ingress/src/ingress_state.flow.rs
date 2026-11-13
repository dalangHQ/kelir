// see: cans/plugins/wire-ingress.md#Connection-parking-on-suspend
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use kelir_interface::driver_interface::{ExternalSuspendKind, SuspendReason};
use kelir_interface::errno_types::Errno;
use kelir_interface::handles_lifecycle::VmHandle;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::plugin_model::{
    KelirPlugin, LifecycleParticipant, MountScope, NoDeps, ParkedConnectionReset, PipelineBinder,
    PluginDeps, VirtioDevice,
};
use kelir_interface::protocol_wire::{Channel, FilterAction, StreamKind, SuspendUntil};

use crate::{
    BacklogConfig, BacklogOverflow, L4ForwardFrame, L4_FORWARD_HEADER_LEN,
    L4_FORWARD_STREAM_KIND, ListenConfig, MAX_PARKED_BYTES, ParkOutcome, ParkedDrain,
    ParkingConfig, WakeOnPacketConfig, WireIngressConfig,
};

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
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParkedConnection {
    target_port: u16,
    buffered: Vec<u8>,
}

// see: cans/plugins/wire-ingress.md#Connection-parking-on-suspend
#[derive(Debug, Default)]
struct IngressState {
    parked: Mutex<HashMap<u64, ParkedConnection>>,
    pending_wake: Mutex<bool>,
    parking: Mutex<bool>,
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
                            Errno::Eproto,
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
        if self.is_parking()? {
            let mut pending_wake = self
                .pending_wake
                .lock()
                .map_err(|_| PluginError::Halt("wire-ingress wake flag poisoned".into()))?;
            *pending_wake = true;
        }
        Ok(ParkOutcome::Parked)
    }

    fn is_parking(&self) -> Result<bool, PluginError> {
        self.parking
            .lock()
            .map_err(|_| PluginError::Halt("wire-ingress parking flag poisoned".into()))
            .map(|flag| *flag)
    }

    fn begin_parking(&self) -> Result<(), PluginError> {
        *self
            .parking
            .lock()
            .map_err(|_| PluginError::Halt("wire-ingress parking flag poisoned".into()))? = true;
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
            .parking
            .lock()
            .map_err(|_| PluginError::Halt("wire-ingress parking flag poisoned".into()))? = false;
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

    pub fn is_parking(&self) -> Result<bool, PluginError> {
        self.state.is_parking()
    }

    // see: cans/plugins/wire-ingress.md#Connection-parking-on-suspend
    pub async fn suspend_for_parking(&self, vm: &VmHandle, path: &str) -> Result<(), PluginError> {
        vm.suspend(
            SuspendReason::External(ExternalSuspendKind::WireConnectionParking),
            path,
        )
        .await?;
        self.state.begin_parking()
    }

    // see: cans/plugins/wire-ingress.md#Wakes-target-microVM-via-SessionWakeHandle::resume
    pub async fn resume_parked(
        &self,
        vm: &VmHandle,
        path: &str,
    ) -> Result<Vec<ParkedDrain>, PluginError> {
        if !self.state.take_pending_wake()? {
            return Ok(Vec::new());
        }
        vm.resume(path).await?;
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

    // see: cans/plugins/wire-ingress.md#Ingress-stream-framing-on-stream-kind-L4-FORWARD-(mux-0x0001):
    // TODO (L3) [wire_plane]: core dispatches L4_FORWARD mux streams into this handler; kelir-interface exposes no mux-kind binder yet; slop: no mux-kind binder, L4_FORWARD never dispatched, handler dead
    pub fn on_l4_forward_frame(&self, payload: &mut [u8]) -> Result<FilterAction, PluginError> {
        let Some((l4, initial)) = L4ForwardFrame::decode(payload) else {
            return Ok(FilterAction::Drop {
                errno: None,
                reason:
                    "wire-ingress: malformed L4_FORWARD frame header dropped at the ingress stream"
                        .into(),
            });
        };
        if !self.router.is_parking()? {
            return Ok(FilterAction::Forward);
        }
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

    pub fn reset_parked_connections(&self) -> Result<Vec<u64>, PluginError> {
        self.router.reset_parked_connections()
    }

    // see: cans/plugins/wire-ingress.md#Wakes-target-microVM-via-SessionWakeHandle::resume
    pub async fn suspend_for_parking(&self, vm: &VmHandle, path: &str) -> Result<(), PluginError> {
        self.router.suspend_for_parking(vm, path).await
    }

    // see: cans/plugins/wire-ingress.md#Wakes-target-microVM-via-SessionWakeHandle::resume
    pub async fn resume_parked(
        &self,
        vm: &VmHandle,
        path: &str,
    ) -> Result<Vec<ParkedDrain>, PluginError> {
        self.router.resume_parked(vm, path).await
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
        // see: cans/plugins/wire-ingress.md#Ingress-stream-framing-on-stream-kind-L4-FORWARD-(mux-0x0001):
        MountScope::default()
            .with_devices([VirtioDevice::Vsock])
            .with_stream_kinds([L4_FORWARD_STREAM_KIND])
    }
    fn register_interceptors(&self, pipeline: &mut dyn PipelineBinder) {
        // see: cans/plugins/wire-ingress.md#Connection-parking-on-suspend
        pipeline.bind_parked_connection_reset(Arc::new(self.router.clone()));
    }
}

impl LifecycleParticipant for WireIngressPlugin {}

// see: cans/02-plugin-model.md#KelirPlugin-trait
impl PluginDeps for WireIngressPlugin {
    type Deps = NoDeps;
}
