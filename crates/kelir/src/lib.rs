// see: cans/01-architecture.md#Host-runtime
#![forbid(unsafe_code)]

// see: cans/artifacts/config/project-structure.txt
pub use kelir_core::*;
pub use kelir_interface as interface;

pub mod builder;
pub mod driver;
pub mod handler_ctx;
pub mod planes;
pub mod plugin;
pub mod plugins;
pub mod protocol;
pub mod sandbox;
pub mod session;
pub mod telemetry;

pub use builder::{
    CompositePlugin, KelirBuilder, PostExecAction, SyntheticFile, TrapFilter, handler,
};
pub use handler_ctx::HandlerCtx;
pub use kelir_core::planes::checkpoint::StepSlot;
pub use kelir_interface::driver::{SnapshotDescriptor, VmConfig, WatchdogAction};
pub use kelir_interface::protocol::Verdict;
pub use planes::storage::{FsReconcileRequest, ReconcileStats};
pub use plugins::ledger::FinalizeRequest;
pub use sandbox::Sandbox;
pub use session::{
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
#[cfg(feature = "plugin-profile")]
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
