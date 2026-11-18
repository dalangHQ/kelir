// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#Userspace-network-stack
// see: cans/07-wire-plane.md#Synthetic-DNS-gateway
use std::sync::{Arc, Mutex};

use kelir_interface::errno_types::Errno;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::protocol_wire::FilterAction;
use kelir_interface::wire::dns::{DnsOps, DnsQuery, DnsResponse};
use kelir_interface::wire::l4::{L4DivertOps, L4Stream};
use kelir_interface::wire::net::{
    FrameDirection, LinkLocalRoute, NetOps, SinkholeConfig, SinkholeRule, WireWiring,
    link_local_route,
};
use kelir_interface::wire::outbox::{OutboxBuffer, OutboxDeferralConfig, OutboxFrame};
use kelir_interface::wire::udp_nat::UdpNatOps;
use kelir_interface::wire::{ConnectionParking, ConnectionParkingConfig};

use super::unbound_ops::{UnboundDnsOps, UnboundL4DivertOps, UnboundNetOps, UnboundUdpNatOps};

// see: cans/07-wire-plane.md#Port-forwarding
#[derive(Debug, thiserror::Error)]
pub enum WireError {
    #[error("host port {0} already forwarded (EADDRINUSE)")]
    PortOccupied(u16),
}

impl From<WireError> for PluginError {
    fn from(error: WireError) -> Self {
        PluginError::Halt(error.to_string())
    }
}

pub struct WirePlane {
    net_ops: Arc<dyn NetOps>,
    dns_ops: Arc<dyn DnsOps>,
    udp_nat_ops: Arc<dyn UdpNatOps>,
    l4_ops: Arc<dyn L4DivertOps>,
    port_forwards: Arc<Mutex<std::collections::HashMap<u16, u16>>>,
    host_listeners:
        Arc<Mutex<std::collections::HashMap<u16, kelir_transport::host_bind::HostListener>>>,
    metrics: Arc<kelir_interface::telemetry::MetricRegistry>,
    wiring: Option<WireWiring>,
    outbox: Arc<Mutex<OutboxBuffer>>,
    parking: Arc<Mutex<ConnectionParking>>,
}

impl Default for WirePlane {
    fn default() -> Self {
        Self::new()
    }
}

// see: cans/07-wire-plane.md#Wire-architecture
pub type WireOps = (
    Arc<dyn NetOps>,
    Arc<dyn DnsOps>,
    Arc<dyn UdpNatOps>,
    Arc<dyn L4DivertOps>,
);

impl WirePlane {
    pub fn new() -> Self {
        Self::with_ops(
            Arc::new(UnboundNetOps),
            Arc::new(UnboundDnsOps),
            Arc::new(UnboundUdpNatOps),
            Arc::new(UnboundL4DivertOps),
        )
    }

    pub fn with_ops(
        net_ops: Arc<dyn NetOps>,
        dns_ops: Arc<dyn DnsOps>,
        udp_nat_ops: Arc<dyn UdpNatOps>,
        l4_ops: Arc<dyn L4DivertOps>,
    ) -> Self {
        Self {
            net_ops,
            dns_ops,
            udp_nat_ops,
            l4_ops,
            port_forwards: Arc::new(Mutex::new(std::collections::HashMap::new())),
            host_listeners: Arc::new(Mutex::new(std::collections::HashMap::new())),
            metrics: Arc::new(kelir_interface::telemetry::MetricRegistry::new()),
            wiring: None,
            outbox: Arc::new(Mutex::new(OutboxBuffer::default())),
            parking: Arc::new(Mutex::new(ConnectionParking::default())),
        }
    }

    // see: cans/07-wire-plane.md#Egress-side-effect-outbox
    pub fn with_outbox_config(self, config: OutboxDeferralConfig) -> Self {
        if let Ok(mut outbox) = self.outbox.lock() {
            *outbox = OutboxBuffer::new(config);
        }
        self
    }

    // see: cans/07-wire-plane.md#Egress-side-effect-outbox
    pub fn buffer_outbox(&self, payload: Vec<u8>, irreversible: bool) -> Result<u64, PluginError> {
        let mut outbox = self
            .outbox
            .lock()
            .map_err(|_| PluginError::Halt("wire outbox slot poisoned".into()))?;
        outbox
            .buffer(payload, irreversible)
            .map_err(|error| PluginError::Halt(error.to_string()))
    }

    // see: cans/07-wire-plane.md#Egress-side-effect-outbox
    pub fn release_outbox_on_step(&self) -> Vec<OutboxFrame> {
        self.outbox
            .lock()
            .map(|mut outbox| outbox.release_verified())
            .unwrap_or_default()
    }

    // see: cans/07-wire-plane.md#Egress-side-effect-outbox
    pub fn purge_outbox_on_rollback(&self) -> Vec<OutboxFrame> {
        self.outbox
            .lock()
            .map(|mut outbox| outbox.purge_rollback())
            .unwrap_or_default()
    }

    // see: cans/plugins/wire-ingress.md#Connection-parking-on-suspend
    pub fn with_parking_config(self, config: ConnectionParkingConfig) -> Self {
        if let Ok(mut parking) = self.parking.lock() {
            *parking = ConnectionParking::new(config);
        }
        self
    }

    // see: cans/plugins/wire-ingress.md#Connection-parking-on-suspend
    pub fn parking(&self) -> Arc<Mutex<ConnectionParking>> {
        Arc::clone(&self.parking)
    }

    pub fn ops(&self) -> WireOps {
        (
            Arc::clone(&self.net_ops),
            Arc::clone(&self.dns_ops),
            Arc::clone(&self.udp_nat_ops),
            Arc::clone(&self.l4_ops),
        )
    }

    // see: cans/07-wire-plane.md#DNS-policy-filter
    // L3-GAP: [wire_plane]: the ssh agent bridge mounts, vault placeholders hydrate, and l4 proxies open host listeners; slop: agent bridge unmounted, placeholders dry, listeners closed
    pub fn with_wiring(self, wiring: WireWiring) -> Self {
        self.dns_ops.apply_wiring(&wiring);
        self.net_ops.apply_wiring(&wiring);
        Self {
            wiring: Some(wiring),
            ..self
        }
    }

    // see: cans/07-wire-plane.md#Sinkhole-rules
    pub(crate) fn wiring_for_network_off(
        wiring: Option<WireWiring>,
        network_off: bool,
    ) -> WireWiring {
        match wiring {
            Some(mut wiring) => {
                if network_off {
                    wiring.sinkhole.ensure_drop_all();
                }
                wiring
            }
            None => WireWiring {
                sinkhole: SinkholeConfig {
                    rules: if network_off {
                        vec![SinkholeRule {
                            host: "*".to_string(),
                            allow: false,
                        }]
                    } else {
                        Vec::new()
                    },
                },
                ..Default::default()
            },
        }
    }

    pub fn with_metrics(
        mut self,
        metrics: Arc<kelir_interface::telemetry::MetricRegistry>,
    ) -> Self {
        self.metrics = metrics;
        self
    }

    pub fn metrics(&self) -> &Arc<kelir_interface::telemetry::MetricRegistry> {
        &self.metrics
    }

    // see: cans/07-wire-plane.md#DNS-policy-filter
    pub fn wiring(&self) -> Option<&WireWiring> {
        self.wiring.as_ref()
    }

    fn record_drop(&self, action: &kelir_interface::protocol_wire::FilterAction) {
        if matches!(
            action,
            kelir_interface::protocol_wire::FilterAction::Drop { .. }
        ) {
            self.metrics.record_drop();
        }
    }

    // see: cans/07-wire-plane.md#Port-forwarding
    pub fn forward_port(
        &self,
        guest_port: u16,
        host_port: Option<u16>,
    ) -> Result<u16, kelir_interface::plugin_model::PluginError> {
        let mut forwards = self.port_forwards.lock().map_err(|_| {
            kelir_interface::plugin_model::PluginError::Halt("port forward table poisoned".into())
        })?;
        let mut listeners = self.host_listeners.lock().map_err(|_| {
            kelir_interface::plugin_model::PluginError::Halt("host listener table poisoned".into())
        })?;
        let bind_port = match host_port.filter(|port| *port != 0) {
            Some(explicit) => {
                if forwards.contains_key(&explicit) {
                    return Err(WireError::PortOccupied(explicit).into());
                }
                explicit
            }
            None => 0,
        };
        let listener =
            kelir_transport::host_bind::bind_host_listener(bind_port).map_err(|error| {
                if bind_port != 0 && error.kind() == std::io::ErrorKind::AddrInUse {
                    WireError::PortOccupied(bind_port).into()
                } else {
                    kelir_interface::plugin_model::PluginError::Halt(format!(
                        "host port {bind_port} bind: {error}"
                    ))
                }
            })?;
        let bound = listener.port();
        forwards.insert(bound, guest_port);
        listeners.insert(bound, listener);
        Ok(bound)
    }

    // see: cans/07-wire-plane.md#Port-forwarding
    pub fn forwarded_guest_port(&self, host_port: u16) -> Result<Option<u16>, PluginError> {
        let forwards = self
            .port_forwards
            .lock()
            .map_err(|_| PluginError::Halt("port forward table poisoned".into()))?;
        Ok(forwards.get(&host_port).copied())
    }

    pub fn process_inbound_frame(
        &mut self,
        frame: &mut [u8],
    ) -> Result<
        kelir_interface::protocol_wire::FilterAction,
        kelir_interface::plugin_model::PluginError,
    > {
        let action = self
            .net_ops
            .intercept_ethernet_frame(FrameDirection::Inbound, frame)?;
        self.record_drop(&action);
        Ok(self.surface_action(action))
    }

    pub fn process_outbound_frame(
        &mut self,
        frame: &mut [u8],
    ) -> Result<
        kelir_interface::protocol_wire::FilterAction,
        kelir_interface::plugin_model::PluginError,
    > {
        match link_local_route(frame, FrameDirection::Outbound) {
            Some(LinkLocalRoute::Drop) => {
                let action = FilterAction::Drop {
                    errno: Errno::Eperm,
                    reason: "link-local 169.254.0.0/16 quarantined pre-policy".into(),
                };
                self.record_drop(&action);
                return Ok(action);
            }
            None => {}
        }
        let action = self
            .net_ops
            .intercept_ethernet_frame(FrameDirection::Outbound, frame)?;
        self.record_drop(&action);
        Ok(self.surface_action(action))
    }

    // see: cans/07-wire-plane.md#Frame-drop-and-forward-semantics
    // L3-GAP: [wire_plane]: vCPU loop must act on this; slop: surfaced action never acted on, vCPU ignores verdict
    pub fn surface_action(
        &self,
        action: kelir_interface::protocol_wire::FilterAction,
    ) -> kelir_interface::protocol_wire::FilterAction {
        if matches!(
            action,
            kelir_interface::protocol_wire::FilterAction::Suspend { .. }
        ) {
            tracing::warn!("FilterAction::Suspend returned without consumer");
        }
        action
    }

    pub fn divert_l4(&self, guest_port: u16, host_target: String) -> Result<L4Stream, PluginError> {
        self.l4_ops.divert_l4(guest_port, host_target)
    }

    // see: cans/07-wire-plane.md#Synthetic-DNS-gateway
    // see: cans/07-wire-plane.md#DNS-policy-filter
    pub fn resolve_query(
        &self,
        query: &mut DnsQuery,
    ) -> Result<DnsResponse, kelir_interface::plugin_model::PluginError> {
        self.dns_ops.resolve_query(query)
    }

    pub fn resolve_synthetic_dns(
        &self,
        payload: &[u8],
    ) -> Result<DnsResponse, kelir_interface::plugin_model::PluginError> {
        self.dns_ops.resolve_synthetic_dns(payload)
    }

    // see: cans/07-wire-plane.md#UDP-socket-reaper
    pub fn udp_nat_alloc(
        &self,
        guest_source_port: u16,
        now_unix: u64,
    ) -> Result<(), kelir_interface::plugin_model::PluginError> {
        self.udp_nat_ops.udp_nat_alloc(guest_source_port, now_unix)
    }

    // see: cans/07-wire-plane.md#UDP-socket-reaper
    pub fn udp_nat_slab_used(&self) -> Result<usize, PluginError> {
        self.udp_nat_ops.udp_nat_slab_used()
    }

    // see: cans/07-wire-plane.md#UDP-socket-reaper
    pub fn reap_stale_udp_sockets(
        &self,
        now_unix: u64,
    ) -> Result<usize, kelir_interface::plugin_model::PluginError> {
        self.udp_nat_ops.reap_stale_udp_sockets(now_unix)
    }

    // see: cans/08-lifecycle.md#Rollback
    // see: cans/07-wire-plane.md#Rollback-interaction
    // see: cans/07-wire-plane.md#Socket-lifecycle
    pub fn quiesce(&self) -> Result<(), PluginError> {
        // L3-GAP: [wp_03_rollback_socket_termination_econnreset]: close live host-side L4 sockets with SO_LINGER zero, emit RST to peers, drain splicing buffers; slop: rollback Ok, host sockets open, no RST, send succeeds
        self.udp_nat_ops.quiesce()?;
        let mut forwards = self
            .port_forwards
            .lock()
            .map_err(|_| PluginError::Halt("port forward table poisoned".into()))?;
        forwards.clear();
        let mut listeners = self
            .host_listeners
            .lock()
            .map_err(|_| PluginError::Halt("host listener table poisoned".into()))?;
        listeners.clear();
        Ok(())
    }
}
