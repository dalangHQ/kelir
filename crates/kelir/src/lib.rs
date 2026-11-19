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
#[path = "sandbox_runtime.flow.rs"]
pub mod sandbox_runtime;
#[path = "session_lifecycle.flow.rs"]
pub mod session_lifecycle;
pub mod telemetry_attest;
#[path = "trap_filter.flow.rs"]
pub mod trap_filter;

pub use builder_scopes::{
    ActionsScope, ConsoleScope, DenyScope, ExecScope, FsScope, FsScopeConfig, HostScope,
    LifecycleScope, NetScope, ParamsScope, ReadRules, RuleScope, ScopeRules, StreamsScope,
    TelemetryScope, TrapScope, VsockScope, WriteRules,
};
pub use builder_sdk::{Host, KelirBuilder, Plugin, Sealed, Unsealed};
pub use hlist_sdk::{HCons, HList, HNil, NoParams};
pub use kelir_interface::checkpoint::PidLineage;
pub use kelir_interface::driver_interface::{SnapshotDescriptor, VmConfig, WatchdogAction};
pub use kelir_interface::handlers_plugin::Has;
pub use kelir_interface::plugin_model::{HandlerCtx, LifecycleEvent};
pub use kelir_interface::protocol_wire::Verdict;
pub use kelir_interface::storage::CommitStats;
pub use kelir_interface::telemetry::CausalDelta;
pub use planes::storage_plane::FsReconcileRequest;
pub use trap_filter::TrapFilter;
// see: cans/plugins/tool-guard.md#Host-extensible-hook-pipeline
pub use async_trait::async_trait;
#[cfg(feature = "plugin-tool-guard")]
pub use builder_sdk::ToolGuardExt;
pub use ciborium::value::Value as CborValue;
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

// see: cans/03-sdk-architecture.md#Full-surface-examples
pub fn block_on<F: Future<Output = T>, T>(future: F) -> std::io::Result<T> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    Ok(runtime.block_on(future))
}

// see: cans/plugins/anomaly.md#Plugin-anomaly
#[cfg(feature = "plugin-anomaly")]
pub use plugin_anomaly::{
    AnomalyConfig, AnomalyCtx, AnomalyDetectionService, AnomalyHandle, AnomalyPlugin,
    FeatureVector, TelemetryRing, TelemetryRingConfig, TelemetrySample,
};
// see: cans/plugins/chaos.md#Plugin-chaos
#[cfg(feature = "plugin-chaos")]
pub use plugin_chaos::{
    ChaosConfig, ChaosPlugin, FaultDirection, FaultInjectionService, NetworkFaultRule,
    StorageFaultRule,
};
#[cfg(feature = "plugin-container")]
pub use plugin_container::{ContainerConfig, ContainerCtx, ContainerHandle, ContainerPlugin};
#[cfg(feature = "plugin-db-proxy")]
pub use plugin_db_proxy::{
    DbProxyConfig, DbProxyPlugin, EphemeralCredentials, NetL7SqlExt, PgSchemaRewrite,
    PgWireRewrite, SqlProxyCtx, SqlProxyHandle, SqlReceiptExt,
};
// see: cans/plugins/egress-proxy.md#Plugin-egress-proxy
#[cfg(feature = "plugin-egress-proxy")]
pub use plugin_egress_proxy::{EgressProxyConfig, EgressProxyPlugin};
// see: cans/plugins/l7-http.md#Plugin-l7-http
#[cfg(feature = "plugin-l7-http")]
pub use plugin_l7_http::{L7HttpConfig, L7HttpPlugin};
// see: cans/plugins/ledger.md#Plugin-ledger
#[cfg(feature = "plugin-ledger")]
pub use plugin_ledger::{
    DualHash, FinalReceipt, FinalizeRequest, HashingStandard, LedgerConfig, LedgerCtx,
    LedgerHandle, LedgerPlugin, LedgerSink, MerkleChain, MerkleWalConfig, OtelConfig,
    ReceiptConfig, SignedTreeHead, WitnessClient, WitnessCosignature,
};
#[cfg(feature = "plugin-mcp")]
pub use plugin_mcp::{McpConfig, McpCtx, McpEndpointService, McpPlugin};
// see: cans/plugins/mesh.md#Plugin-mesh
#[cfg(feature = "plugin-mesh")]
pub use plugin_mesh::{
    MeshConfig, MeshCtx, MeshHandle, MeshPlugin, PeerIdentity, PeerListener,
    TraceParentPropagation, VectorClock,
};
pub use plugin_profile::{ProfileConfig, ProfileCtx, ProfilePlugin};
// see: cans/plugins/redaction.md#Plugin-redaction
#[cfg(feature = "plugin-redaction")]
pub use plugin_redaction::{MaskReadConfig, RedactionConfig, RedactionPlugin};
// see: cans/plugins/replay.md#Plugin-replay
#[cfg(feature = "plugin-replay")]
pub use plugin_replay::{ReplayConfig, ReplayCtx, ReplayPlugin};
// see: cans/plugins/step-eval.md#Plugin-step-eval
pub use plugin_step_eval::{
    PostExecAction, StepEvalConfig, StepEvalPlugin, StepEvaluationCtx, StepEvaluationService,
    StepEvaluator, StepVerdict,
};
// see: cans/plugins/supply-chain.md#Plugin-supply-chain
#[cfg(feature = "plugin-supply-chain")]
pub use plugin_supply_chain::{
    AuthorizedJitEngine, JitValidationService, PackageGuardConfig, SupplyChainConfig,
    SupplyChainCtx, SupplyChainPlugin, WxJitConfig,
};
// see: cans/plugins/taint.md#Plugin-taint
#[cfg(feature = "plugin-taint")]
pub use plugin_taint::{TaintPlugin, TaintTrackingService};
// see: cans/plugins/terminal-guard.md#Plugin-terminal-guard
#[cfg(feature = "plugin-terminal-guard")]
pub use plugin_terminal_guard::{TerminalGuardConfig, TerminalGuardPlugin};
// see: cans/plugins/token-shield.md#Plugin-token-shield
#[cfg(feature = "plugin-token-shield")]
pub use plugin_token_shield::{TokenShieldConfig, TokenShieldPlugin};
#[cfg(feature = "plugin-tool-guard")]
pub use plugin_tool_guard::{ToolGuardConfig, ToolGuardCtx, ToolGuardPlugin, ToolSpec};
#[cfg(feature = "plugin-transaction")]
pub use plugin_transaction::{
    TransactionConfig, TransactionCtx, TransactionHandle, TransactionPlugin, Tx,
};
#[cfg(feature = "plugin-vault")]
pub use plugin_vault::{CredentialBrokerCtx, VaultConfig, VaultPlugin};
#[cfg(feature = "plugin-wire-ingress")]
pub use plugin_wire_ingress::{
    IngressRoutingService, SpliceRoute, WireIngressConfig, WireIngressPlugin,
};
