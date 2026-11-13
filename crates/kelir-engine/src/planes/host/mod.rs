// see: cans/01-architecture.md#Host-plane-subsystem
// see: cans/01-architecture.md#Host-memory-pressure-watcher
// see: cans/08-lifecycle.md#Host-crash-reaper
#[path = "plane.flow.rs"]
mod plane;

pub use plane::HostPlane;
