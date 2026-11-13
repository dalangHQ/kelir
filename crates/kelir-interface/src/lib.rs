#![forbid(unsafe_code)]

// TODO (L3) [protocol_mux]: tests/unit_codec.rs + tests/unit_traits.rs per cans/artifacts/config/project-structure.txt.; slop: stub tests assert Ok(()), no codec wire bytes
pub mod chaos_types;
pub mod codec_protocol;
pub mod credential_vault;
pub mod driver_interface;
pub mod errno_types;
pub mod error_envelope;
pub mod frame_layout_views;
pub mod fusion_plugin;
pub mod handlers_plugin;
pub mod handles_lifecycle;
pub mod hex_codec;
pub mod lifecycle_state;
pub mod mount_scope;
pub mod plugin_model;
pub mod protocol_wire;
pub mod runtime_block;
pub mod spawn_transport;
pub mod telemetry;
pub mod vsock_mesh;
