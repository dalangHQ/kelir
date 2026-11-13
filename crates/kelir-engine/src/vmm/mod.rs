// see: cans/01-architecture.md#Host-runtime
// see: cans/01-architecture.md#Driver-backends
#[path = "router.flow.rs"]
mod router;

pub use router::DriverRouter;
