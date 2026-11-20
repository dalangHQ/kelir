// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#UDP-socket-reaper
use std::sync::{Arc, Mutex};

use kelir_interface::plugin_model::PluginError;
use kelir_interface::wire::udp_nat::{UdpNatEntry, UdpNatOps};

// TODO (L3) [wire_plane]: remove this allow when the stubbed L3 callers land and read this item again.; slop: allow outlives the L3 callers, dead-code lint stays suppressed and the stubbed item looks wired while nothing reads it.
#[allow(dead_code)]
pub(crate) struct RealUdpNatOps {
    udp_nat_slab: Mutex<Vec<Option<UdpNatEntry>>>,
}

impl RealUdpNatOps {
    pub(crate) fn new() -> Self {
        Self {
            udp_nat_slab: Mutex::new(Vec::new()),
        }
    }
}

impl Default for RealUdpNatOps {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn build_udp_nat_ops() -> Arc<dyn UdpNatOps> {
    Arc::new(RealUdpNatOps::new())
}

impl UdpNatOps for RealUdpNatOps {
    // see: cans/07-wire-plane.md#UDP-socket-reaper
    fn udp_nat_alloc(&self, _guest_source_port: u16, _now_unix: u64) -> Result<(), PluginError> {
        // L3-GAP: [wire_plane]: open the per-source-port host UDP socket and store its descriptor in the slab entry (Full Cone endpoint independence); slop: slab entry without host socket, endpoint independence broken
        // TODO (L3) [wire_plane]: allocate or refresh the slab entry for guest_source_port with its host UDP socket.; slop: slab entry never allocated, guest UDP packets drop with no host socket bound.
        Err(PluginError::Halt(
            "TODO (L3): UdpNatOps::udp_nat_alloc needs the per-port host UDP socket".into(),
        ))
    }

    // see: cans/07-wire-plane.md#UDP-socket-reaper
    fn udp_nat_slab_used(&self) -> Result<usize, PluginError> {
        // TODO (L3) [wire_plane]: count the occupied slots in the UDP NAT slab.; slop: slab usage always reads zero, exhaustion never detected and new allocations silently fail.
        Err(PluginError::Halt(
            "TODO (L3): UdpNatOps::udp_nat_slab_used needs the slab occupancy count".into(),
        ))
    }

    // see: cans/07-wire-plane.md#UDP-socket-reaper
    fn reap_stale_udp_sockets(&self, _now_unix: u64) -> Result<usize, PluginError> {
        // L3-GAP: [wire_plane]: close(2) the host UDP socket descriptor before releasing the slab slot; slop: slab released with host fd open, descriptor leaks
        // TODO (L3) [wire_plane]: close and release every slab entry idle past the timeout and return the reaped count.; slop: stale sockets never reaped, the slab fills with dead entries and live guests lose NAT ports.
        Err(PluginError::Halt(
            "TODO (L3): UdpNatOps::reap_stale_udp_sockets needs the stale socket reaper".into(),
        ))
    }

    fn quiesce(&self) -> Result<(), PluginError> {
        // TODO (L3) [wire_plane]: close every host UDP socket and clear the NAT slab.; slop: quiesce leaves host sockets open, guest UDP keeps flowing after teardown.
        Err(PluginError::Halt(
            "TODO (L3): UdpNatOps::quiesce needs the NAT slab teardown".into(),
        ))
    }
}
