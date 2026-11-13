// see: cans/01-architecture.md#Host-runtime
#![forbid(unsafe_code)]
// TODO (L3) [protocol_mux]: wire CustomActionEntry/StreamKindServiceEntry/SdkStreamServiceSink into live stream dispatch; slop: stream services registered but never consumed
#![allow(dead_code)]

// TODO (L3) [plugin_engine]: tests/ facade integration tests per cans/artifacts/config/project-structure.txt.; slop: facade tests assert Ok(()), no session exercised
pub use kelir_interface as interface;
pub use kelir_interface::driver_interface;

pub mod builder_adapters;
pub mod builder_plugin;
pub mod builder_scopes;
pub mod builder_sdk;
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
pub use builder_sdk::{
    Host, KelirBuilder, LifecycleMut, Plugin, PostExecAction, Sealed, StepVerdict, TrapFilter,
    Unsealed,
};
pub use hlist_sdk::{HCons, HList, HNil, NoParams};
pub use kelir_interface::checkpoint::{PidLineage, StepSlot};
pub use kelir_interface::driver_interface::{SnapshotDescriptor, VmConfig, WatchdogAction};
pub use kelir_interface::handlers_plugin::Has;
pub use kelir_interface::plugin_model::HandlerCtx;
pub use kelir_interface::protocol_wire::Verdict;
pub use kelir_interface::storage::CommitStats;
// see: cans/01-architecture.md#Host-runtime
pub use kelir_checkpoint::RealCheckpointOps;
pub use kelir_host::RealHostOps;
pub use kelir_storage::RealFuseOps;
pub use kelir_storage_io::RealStorageIo;
pub use kelir_terminal::RealTerminalOps;
pub use kelir_wire::{RealDnsOps, RealL4DivertOps, RealNetOps, RealUdpNatOps};
pub use planes::storage_plane::{FsReconcileRequest, SyntheticFile};
// see: cans/plugins/tool-guard.md#Host-extensible-hook-pipeline
#[cfg(feature = "plugin-tool-guard")]
pub use builder_sdk::ToolGuardExt;
pub use sandbox_runtime::{Sandbox, SandboxOptions, SandboxProfile};
pub use session_lifecycle::{
    ClockHandle, FsHandle, Session, SpawnResult, StepStack, TerminalHandle, VmHandle, VsockChannel,
    VsockHandle,
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
