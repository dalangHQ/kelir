// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/01-architecture.md#Foreign-runtime-embedding
// see: cans/02-plugin-model.md#Foreign-plugin-proxy
#![forbid(unsafe_code)]

#[path = "fusion_plugin.flow.rs"]
pub mod fusion_plugin;
pub mod storage_bridge;
#[path = "handles.flow.rs"]
pub mod handles;
#[path = "builder.flow.rs"]
pub mod builder;
#[path = "handles.run.rs"]
pub mod handles_run;

pub use builder::{
    NativeKelirBuilder, NativeKelirOptions, NativeL4Proxy, NativeMaskRule, NativeStorageConfig,
    NativeSyntheticFile, NativeTerminalConfig, NativeVaultDescriptor, NativeWireConfig,
};
pub use handles::{
    NativeCheckpointHandle, NativeClockHandle, NativeClockSkewOptions, NativeCommitStatsRecord,
    NativeGuestClockRecord, NativeHostHandle, NativePluginContextRecord, NativePluginContextSurface,
    NativeReconcileOptions, NativeSessionHandle, NativeSnapshotDescriptorRecord, NativeStepHandle,
    NativeStorageHandle, NativeTerminalHandle, NativeTerminalReadRecord, NativeVmHandle,
    NativeVsockHandle, NativeWireHandle, relay_socket_path,
};
pub(crate) use handles::envelope_error;
