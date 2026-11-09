#![forbid(unsafe_code)]

// TODO (L3): tests/unit_codec.rs + tests/unit_traits.rs per cans/artifacts/config/project-structure.txt.
pub mod codec_protocol;
pub mod credential_vault;
pub mod driver_interface;
pub mod fusion_plugin;
pub mod handlers_plugin;
pub mod handles_lifecycle;
pub mod plugin_model;
pub mod protocol_wire;
pub mod telemetry;
pub mod vsock_mesh;
