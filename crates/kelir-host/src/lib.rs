// see: cans/01-architecture.md#Host-plane-subsystem
// see: cans/08-lifecycle.md#Host-crash-reaper
#![forbid(unsafe_code)]

#[path = "balloon.run.rs"]
pub mod balloon;
#[path = "pressure.run.rs"]
pub mod pressure;
#[path = "real_host.run.rs"]
pub mod real_host;
#[path = "reaper.run.rs"]
pub mod reaper;

pub use real_host::RealHostOps;
