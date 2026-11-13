// see: cans/01-architecture.md#Host-plane-subsystem
// see: cans/08-lifecycle.md#Host-crash-reaper
#![forbid(unsafe_code)]

#[path = "balloon.run.rs"]
mod balloon;
#[path = "host.flow.rs"]
pub mod host;
#[path = "pressure.run.rs"]
mod pressure;
#[path = "real_host.run.rs"]
mod real_host;
#[path = "reaper.run.rs"]
mod reaper;

pub use crate::host::host_ops;
