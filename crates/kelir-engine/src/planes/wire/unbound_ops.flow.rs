// see: cans/07-wire-plane.md#Wire-architecture
use super::*;
pub(super) struct UnboundNetOps;

impl NetOps for UnboundNetOps {
    fn intercept_ethernet_frame(
        &self,
        _direction: FrameDirection,
        _frame: &mut [u8],
    ) -> Result<FilterAction, PluginError> {
        Err(PluginError::Halt(
            "NetOps unbound at WirePlane::new; bind the kelir-wire ops in the composition root"
                .into(),
        ))
    }

    fn apply_wiring(&self, _wiring: &WireWiring) {}
}

pub(super) struct UnboundDnsOps;

impl DnsOps for UnboundDnsOps {
    fn resolve_query(&self, _query: &mut DnsQuery) -> Result<DnsResponse, PluginError> {
        Err(PluginError::Halt(
            "DnsOps unbound at WirePlane::new; bind the kelir-wire ops in the composition root"
                .into(),
        ))
    }

    fn resolve_synthetic_dns(&self, _payload: &[u8]) -> Result<DnsResponse, PluginError> {
        Err(PluginError::Halt(
            "DnsOps unbound at WirePlane::new; bind the kelir-wire ops in the composition root"
                .into(),
        ))
    }

    fn apply_wiring(&self, _wiring: &WireWiring) {}
}

pub(super) struct UnboundUdpNatOps;

impl UdpNatOps for UnboundUdpNatOps {
    fn udp_nat_alloc(&self, _guest_source_port: u16, _now_unix: u64) -> Result<(), PluginError> {
        Err(PluginError::Halt(
            "UdpNatOps unbound at WirePlane::new; bind the kelir-wire ops in the composition root"
                .into(),
        ))
    }

    fn udp_nat_slab_used(&self) -> Result<usize, PluginError> {
        Err(PluginError::Halt(
            "UdpNatOps unbound at WirePlane::new; bind the kelir-wire ops in the composition root"
                .into(),
        ))
    }

    fn reap_stale_udp_sockets(&self, _now_unix: u64) -> Result<usize, PluginError> {
        Err(PluginError::Halt(
            "UdpNatOps unbound at WirePlane::new; bind the kelir-wire ops in the composition root"
                .into(),
        ))
    }

    fn quiesce(&self) -> Result<(), PluginError> {
        Err(PluginError::Halt(
            "UdpNatOps unbound at WirePlane::new; bind the kelir-wire ops in the composition root"
                .into(),
        ))
    }
}

// TODO (L3) [wire_plane]: WirePlane::new binds fail-closed Unbound*Ops; Agent 5b replaces construction with the kelir-wire ops; slop: unbound ops Halt every frame/DNS/NAT/L4 call until the composition root binds them
pub(super) struct UnboundL4DivertOps;

impl L4DivertOps for UnboundL4DivertOps {
    fn divert_l4(&self, _guest_port: u16, _host_target: String) -> Result<L4Stream, PluginError> {
        Err(PluginError::Halt(
            "L4DivertOps unbound at WirePlane::new; bind the kelir-wire ops in the composition root"
                .into(),
        ))
    }
}
