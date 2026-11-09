// see: cans/01-architecture.md#Host-runtime
#![forbid(unsafe_code)]

// TODO (L3): tests/ facade integration tests per cans/artifacts/config/project-structure.txt.
pub use kelir_core::*;
pub use kelir_interface as interface;

pub mod builder_sdk;
pub mod driver_interface;
pub mod hlist_sdk;
pub mod planes;
pub mod plugin_model;
pub mod plugins;
pub mod protocol_wire;
pub mod sandbox_runtime;
pub mod session_lifecycle;
pub mod telemetry_attest;

pub use builder_sdk::{KelirBuilder, PostExecAction, Sealed, TrapFilter, Unsealed, handler};
pub use hlist_sdk::{HCons, HList, HNil, NoParams};
pub use kelir_core::planes::checkpoint::{PidLineage, StepSlot};
pub use kelir_core::planes::storage::storage_reconciler::CommitStats;
pub use kelir_interface::driver_interface::{SnapshotDescriptor, VmConfig, WatchdogAction};
pub use kelir_interface::handlers_plugin::Has;
pub use kelir_interface::plugin_model::HandlerCtx;
pub use kelir_interface::protocol_wire::Verdict;
pub use planes::storage_plane::{FsReconcileRequest, SyntheticFile};
pub use sandbox_runtime::{Sandbox, SandboxOptions, SandboxProfile};
pub use session_lifecycle::{
    ClockHandle, ExecResult, FsHandle, Session, SpawnBuilder, StepStack, TerminalHandle, VmHandle,
    VsockChannel, VsockHandle, WireHandle,
};

#[cfg(feature = "plugin-chaos")]
pub use plugin_chaos::ChaosPlugin;
#[cfg(feature = "plugin-container")]
pub use plugin_container::ContainerPlugin;
#[cfg(feature = "plugin-db-proxy")]
pub use plugin_db_proxy::DbProxyPlugin;
#[cfg(feature = "plugin-ledger")]
pub use plugin_ledger::LedgerPlugin;
#[cfg(feature = "plugin-mcp")]
pub use plugin_mcp::McpPlugin;
#[cfg(feature = "plugin-mesh")]
pub use plugin_mesh::MeshPlugin;
pub use plugin_profile::ProfilePlugin;
#[cfg(feature = "plugin-replay")]
pub use plugin_replay::ReplayPlugin;
#[cfg(feature = "plugin-supply-chain")]
pub use plugin_supply_chain::SupplyChainPlugin;
#[cfg(feature = "plugin-tool-guard")]
pub use plugin_tool_guard::ToolGuardPlugin;
#[cfg(feature = "plugin-transaction")]
pub use plugin_transaction::TransactionPlugin;
#[cfg(feature = "plugin-vault")]
pub use plugin_vault::VaultPlugin;
#[cfg(feature = "plugin-wire-ingress")]
pub use plugin_wire_ingress::WireIngressPlugin;
