// see: cans/01-architecture.md#Host-plane-subsystem
// see: cans/08-lifecycle.md#Host-crash-reaper
#![forbid(unsafe_code)]

#[path = "balloon.flow.rs"]
mod balloon;
#[path = "host.flow.rs"]
pub mod host;
#[path = "real_host.run.rs"]
mod real_host;
#[path = "reaper.flow.rs"]
mod reaper;
#[path = "reaper.run.rs"]
mod reaper_run;

pub use crate::host::host_ops;
