// see: cans/plugins/replay.md#Plugin-replay
#![forbid(unsafe_code)]

#[path = "replay_state.flow.rs"]
pub mod replay_state;

pub use replay_state::{ReplayCtx, ReplayPlugin};

// see: cans/artifacts/examples/replay.rs#ReplayConfig
// see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.replay
#[derive(Debug, Clone)]
pub struct ReplayConfig {
    pub step_window_instructions: u64,
    pub pinned_clocksource: String,
    pub deterministic_rng_seed: u64,
    pub network_isolation: bool,
}
