#![forbid(unsafe_code)]
// see: cans/07-wire-plane.md#Wire-architecture
#[path = "dns.run.rs"]
pub mod dns;
#[path = "l4.run.rs"]
pub mod l4;
#[path = "smoltcp.run.rs"]
pub mod smoltcp_stack;
#[path = "udp_nat.run.rs"]
pub mod udp_nat;

pub use crate::dns::RealDnsOps;
pub use crate::l4::RealL4DivertOps;
pub use crate::smoltcp_stack::RealNetOps;
pub use crate::udp_nat::RealUdpNatOps;
