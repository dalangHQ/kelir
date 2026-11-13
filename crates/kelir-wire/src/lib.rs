#![forbid(unsafe_code)]
// see: cans/07-wire-plane.md#Wire-architecture
#[path = "dns.run.rs"]
mod dns;
#[path = "l4.run.rs"]
mod l4;
#[path = "smoltcp.run.rs"]
mod smoltcp_stack;
#[path = "udp_nat.run.rs"]
mod udp_nat;
#[path = "wire.flow.rs"]
pub mod wire;

pub use crate::wire::{dns_ops, l4_ops, net_ops, udp_nat_ops};
