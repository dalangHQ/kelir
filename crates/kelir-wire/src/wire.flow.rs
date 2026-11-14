// see: cans/07-wire-plane.md#Wire-architecture
use std::sync::Arc;

use kelir_interface::wire::dns::DnsOps;
use kelir_interface::wire::l4::L4DivertOps;
use kelir_interface::wire::net::NetOps;
use kelir_interface::wire::udp_nat::UdpNatOps;

use crate::dns::build_dns_ops;
use crate::l4::build_l4_ops;
use crate::smoltcp_stack::build_net_ops;
use crate::udp_nat::build_udp_nat_ops;

pub fn dns_ops() -> Arc<dyn DnsOps> {
    build_dns_ops()
}

pub fn net_ops(dns: Arc<dyn DnsOps>) -> Arc<dyn NetOps> {
    build_net_ops(dns)
}

pub fn udp_nat_ops() -> Arc<dyn UdpNatOps> {
    build_udp_nat_ops()
}

pub fn l4_ops() -> Arc<dyn L4DivertOps> {
    build_l4_ops()
}
