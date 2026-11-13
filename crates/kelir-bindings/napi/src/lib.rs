// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/01-architecture.md#Foreign-runtime-embedding
// see: cans/02-plugin-model.md#Foreign-plugin-proxy
#![forbid(unsafe_code)]

#[path = "builder.flow.rs"]
pub mod builder;
#[path = "fusion_plugin.flow.rs"]
pub mod fusion_plugin;
#[path = "handles.flow.rs"]
pub mod handles;
#[path = "handles.run.rs"]
pub mod handles_run;
pub mod storage_bridge;

pub use builder::{
    NativeKelirBuilder, NativeKelirOptions, NativeL4Proxy, NativeMaskRule, NativeStorageConfig,
    NativeSyntheticFile, NativeTerminalConfig, NativeVaultDescriptor, NativeWireConfig,
};
pub(crate) use handles::envelope_error;
pub use handles::{
    NativeCheckpointHandle, NativeClockHandle, NativeClockSkewOptions, NativeCommitStatsRecord,
    NativeGuestClockRecord, NativeHostHandle, NativePluginContextRecord,
    NativePluginContextSurface, NativeReconcileOptions, NativeSessionHandle,
    NativeSnapshotDescriptorRecord, NativeStepHandle, NativeStorageHandle, NativeTerminalHandle,
    NativeTerminalReadRecord, NativeVmHandle, NativeVsockHandle, NativeWireHandle,
    relay_socket_path,
};
