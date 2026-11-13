// see: cans/01-architecture.md#Axiomatic-foundations
// see: cans/08-lifecycle.md#State-machine
#![forbid(unsafe_code)]
// TODO (L3) [hypervisor_driver]: read DriverRouter.active_driver once the router dispatches to a live backend; slop: router holds a driver it never drives
#![allow(dead_code)]

// see: cans/15-operations.md#Developer-CLI
pub const KELIR_CORE_VERSION: &str = env!("CARGO_PKG_VERSION");

// TODO (L3) [hypervisor_driver]: vmm/router.rs + tests/unit_bus.rs, unit_dispatch.rs, unit_planes.rs per cans/artifacts/config/project-structure.txt; slop: router module missing, layout unproven
pub mod bus;
pub mod dispatch_plugin;
pub mod planes;
pub mod plugin_registry;
#[path = "session_lifecycle.flow.rs"]
pub mod session_lifecycle;
#[path = "session_support.flow.rs"]
pub mod session_support;
#[path = "trap_gate.flow.rs"]
pub mod trap_gate;
#[path = "uart_terminal.flow.rs"]
pub mod uart_terminal;
pub mod vmm;

pub use plugin_registry::PluginRegistration;
pub use session_lifecycle::Session;
