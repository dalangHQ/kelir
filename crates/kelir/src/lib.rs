// see: cans/01-architecture.md#Host-runtime
#![forbid(unsafe_code)]
// L3-GAP: [protocol_mux]: wire CustomActionEntry/StreamKindServiceEntry/SdkStreamServiceSink into live stream dispatch; slop: stream services registered but never consumed
#![allow(dead_code)]

// L3-GAP: [plugin_engine]: tests/ facade integration tests per cans/artifacts/config/project-structure.txt.; slop: facade tests assert Ok(()), no session exercised
pub use kelir_interface as interface;
pub use kelir_interface::driver_interface;

#[path = "builder_adapters.flow.rs"]
pub mod builder_adapters;
#[path = "builder_plugin.flow.rs"]
pub mod builder_plugin;
#[path = "builder_scopes.flow.rs"]
pub mod builder_scopes;
#[path = "builder_sdk.flow.rs"]
pub mod builder_sdk;
#[path = "builder_typestate.flow.rs"]
pub mod builder_typestate;
#[path = "hlist_sdk.flow.rs"]
pub mod hlist_sdk;
pub mod planes;
pub mod plugin_model;
#[path = "profile_schema.flow.rs"]
pub mod profile_schema;
pub mod profile_validate;
#[path = "profile_validate.flow.rs"]
mod profile_validate_flow;
pub mod protocol_wire;
#[path = "runtime.flow.rs"]
pub mod runtime;
#[path = "sandbox_runtime.flow.rs"]
pub mod sandbox_runtime;
#[path = "session_lifecycle.flow.rs"]
pub mod session_lifecycle;
pub mod telemetry_attest;
#[path = "trap_filter.flow.rs"]
pub mod trap_filter;

pub use async_trait::async_trait;
pub use builder_scopes::{
    ActionsScope, ConsoleScope, DenyScope, ExecScope, FsScope, FsScopeConfig, HostScope,
    LifecycleScope, NetScope, ParamsScope, ReadRules, RuleScope, ScopeRules, StreamsScope,
    TelemetryScope, TrapScope, VsockScope, WriteRules,
};
pub use builder_sdk::{Host, KelirBuilder, Plugin, Sealed, Unsealed};
pub use ciborium::value::Value as CborValue;
pub use hlist_sdk::{HCons, HList, HNil, NoParams};
pub use kelir_interface::checkpoint::PidLineage;
pub use kelir_interface::driver_interface::{SnapshotDescriptor, VmConfig, WatchdogAction};
pub use kelir_interface::handlers_plugin::Has;
pub use kelir_interface::plugin_model::{HandlerCtx, LifecycleEvent};
pub use kelir_interface::protocol_wire::Verdict;
pub use kelir_interface::storage::CommitStats;
pub use kelir_interface::telemetry::CausalDelta;
pub use planes::storage_plane::FsReconcileRequest;
pub use profile_schema::EffectiveConfig;
pub use sandbox_runtime::{Sandbox, SandboxOptions, SandboxProfile};
pub use serde;
pub use serde::{Deserialize, Serialize};
pub use serde_json;
pub use session_lifecycle::{
    ClockHandle, FsHandle, IngressChannel, IngressStream, Session, ShmMapping, SpawnOpts,
    SpawnResult, StepStack, TerminalHandle, VmHandle, VsockChannel, VsockHandle,
};
pub use tokio::fs;
pub use tokio::task::JoinSet;
pub use tokio::time;
pub use trap_filter::TrapFilter;

// see: cans/03-sdk-architecture.md#Full-surface-examples
pub fn block_on<F: Future<Output = T>, T>(future: F) -> std::io::Result<T> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    Ok(runtime.block_on(future))
}
