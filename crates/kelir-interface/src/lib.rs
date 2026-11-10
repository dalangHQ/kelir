#![forbid(unsafe_code)]

// TODO (L3): tests/unit_codec.rs + tests/unit_traits.rs per cans/artifacts/config/project-structure.txt.
pub mod chaos_types;
pub mod codec_protocol;
pub mod credential_vault;
pub mod driver_interface;
pub mod error_envelope;
pub mod frame_layout_views;
pub mod fusion_plugin;
pub mod handlers_plugin;
pub mod handles_lifecycle;
pub mod hex_codec;
pub mod lifecycle_state;
pub mod plugin_model;
pub mod protocol_wire;
pub mod runtime_block;
pub mod telemetry;
pub mod vsock_mesh;
