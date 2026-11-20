// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#Userspace-network-stack
// see: cans/07-wire-plane.md#Synthetic-DNS-gateway
#[path = "layout_wire.flow.rs"]
pub(crate) mod layout_wire;
#[path = "plane.flow.rs"]
mod plane;

pub use plane::{WireError, WireOps, WirePlane};
