// see: cans/01-architecture.md#Host-plane-subsystem
#[path = "ops.flow.rs"]
pub mod ops;

pub use ops::{
    BalloonRequest, HostOps, HostReaperKind, PressureEvent, PressureLevel, PressureSource,
    PressureSubscription, ReclaimRequest,
};
