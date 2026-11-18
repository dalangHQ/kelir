// see: cans/plugins/token-shield.md#Plugin-token-shield
#![forbid(unsafe_code)]

#[path = "clamp.run.rs"]
pub mod clamp;
#[path = "service_token_shield.flow.rs"]
pub mod service_token_shield;

pub use service_token_shield::{
    ClampStrategy, ScrollbackClampService, TokenShieldConfig, TokenShieldPlugin,
};
