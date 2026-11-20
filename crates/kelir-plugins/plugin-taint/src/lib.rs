// see: cans/plugins/taint.md#Plugin-taint
#![forbid(unsafe_code)]

#[path = "service_taint.flow.rs"]
pub mod service_taint;
#[path = "state.flow.rs"]
pub mod state;
#[path = "state.run.rs"]
pub mod state_run;

pub use service_taint::{ByteRange, TaintSet, TaintTracker, TaintTrackingService};
pub use state::{TaintPlugin, TaintTrackingCtx};
