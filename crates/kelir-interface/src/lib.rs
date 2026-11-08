#![forbid(unsafe_code)]

// TODO (L3): tests/unit_codec.rs + tests/unit_traits.rs per cans/artifacts/config/project-structure.txt.
pub mod codec;
pub mod credential;
pub mod driver;
pub mod fusion;
pub mod handlers;
pub mod handles;
pub mod plugin;
pub mod protocol;
pub mod service_registry;
pub mod telemetry;
pub mod vsock;
