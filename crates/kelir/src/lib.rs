// see: cans/01-architecture.md#Host-runtime
#![forbid(unsafe_code)]

// TODO (L3): tests/ facade integration tests per cans/artifacts/config/project-structure.txt.
pub use kelir_core::*;
pub use kelir_interface as interface;

pub mod builder_plugin;
pub mod builder_scopes;
pub mod builder_sdk;
pub mod driver_interface;
pub mod hlist_sdk;
pub mod planes;
pub mod plugin_model;
pub mod protocol_wire;
pub mod sandbox_runtime;
pub mod session_lifecycle;
pub mod telemetry_attest;

pub use builder_scopes::{
    ConsoleScope, ExecScope, FsScope, HostScope, LifecycleScope, NetScope, TelemetryScope,
    TrapScope,
};
pub use builder_sdk::{Host, KelirBuilder, Plugin, PostExecAction, Sealed, TrapFilter, Unsealed};
pub use hlist_sdk::{HCons, HList, HNil, NoParams};
pub use kelir_core::planes::checkpoint::{PidLineage, StepSlot};
pub use kelir_core::planes::storage::storage_reconciler::CommitStats;
pub use kelir_interface::driver_interface::{SnapshotDescriptor, VmConfig, WatchdogAction};
pub use kelir_interface::handlers_plugin::Has;
pub use kelir_interface::plugin_model::HandlerCtx;
pub use kelir_interface::protocol_wire::Verdict;
pub use planes::storage_plane::{FsReconcileRequest, SyntheticFile};
// see: cans/plugins/tool-guard.md#Host-extensible-hook-pipeline
#[cfg(feature = "plugin-tool-guard")]
pub use builder_sdk::ToolGuardExt;
pub use sandbox_runtime::{Sandbox, SandboxOptions, SandboxProfile};
pub use session_lifecycle::{
    ClockHandle, FsHandle, Session, SpawnResult, StepStack, TerminalHandle, VmHandle, VsockChannel,
    VsockHandle, WireHandle,
};

// see: cans/plugins/chaos.md#Plugin-chaos
#[cfg(feature = "plugin-chaos")]
pub use plugin_chaos::{
    ChaosConfig, ChaosPlugin, FaultDirection, FaultInjectionService, NetworkFaultRule,
    StorageFaultRule,
};
#[cfg(feature = "plugin-container")]
pub use plugin_container::{ContainerConfig, ContainerCtx, ContainerPlugin};
#[cfg(feature = "plugin-db-proxy")]
pub use plugin_db_proxy::{DbProxyConfig, DbProxyPlugin, SqlProxyCtx};
// see: cans/plugins/ledger.md#Plugin-ledger
#[cfg(feature = "plugin-ledger")]
pub use plugin_ledger::{
    FinalReceipt, FinalizeRequest, LedgerConfig, LedgerCtx, LedgerPlugin, LedgerSink, OtelConfig,
    ReceiptConfig,
};
#[cfg(feature = "plugin-mcp")]
pub use plugin_mcp::{McpCtx, McpEndpointService, McpPlugin};
// see: cans/plugins/mesh.md#Plugin-mesh
#[cfg(feature = "plugin-mesh")]
pub use plugin_mesh::{
    MeshConfig, MeshCtx, MeshPlugin, PeerIdentity, PeerListener, TraceParentPropagation,
};
pub use plugin_profile::{ProfileConfig, ProfileCtx, ProfilePlugin};
// see: cans/plugins/replay.md#Plugin-replay
#[cfg(feature = "plugin-replay")]
pub use plugin_replay::{ReplayConfig, ReplayCtx, ReplayPlugin};
// see: cans/plugins/supply-chain.md#Plugin-supply-chain
#[cfg(feature = "plugin-supply-chain")]
pub use plugin_supply_chain::{
    AuthorizedJitEngine, JitValidationService, PackageGuardConfig, SupplyChainConfig,
    SupplyChainCtx, SupplyChainPlugin, WxJitConfig,
};
#[cfg(feature = "plugin-tool-guard")]
pub use plugin_tool_guard::{ToolGuardConfig, ToolGuardCtx, ToolGuardPlugin, ToolSpec};
#[cfg(feature = "plugin-transaction")]
pub use plugin_transaction::{TransactionConfig, TransactionCtx, TransactionPlugin};
#[cfg(feature = "plugin-vault")]
pub use plugin_vault::{CredentialBrokerCtx, VaultConfig, VaultPlugin};
#[cfg(feature = "plugin-wire-ingress")]
pub use plugin_wire_ingress::{IngressRoutingService, WireIngressConfig, WireIngressPlugin};
