// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#Userspace-network-stack
// see: cans/07-wire-plane.md#Synthetic-DNS-gateway
#[path = "layout_wire.flow.rs"]
pub(crate) mod layout_wire;
#[path = "plane.flow.rs"]
mod plane;
#[path = "unbound_ops.flow.rs"]
mod unbound_ops;

pub use plane::{WireError, WireOps, WirePlane};
