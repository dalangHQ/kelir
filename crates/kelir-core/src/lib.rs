// see: cans/01-architecture.md#Axiomatic-foundations
// see: cans/08-lifecycle.md#State-machine
#![forbid(unsafe_code)]

// TODO (L3): vmm/router.rs + tests/unit_bus.rs, unit_dispatch.rs, unit_planes.rs per cans/artifacts/config/project-structure.txt.
pub mod bus;
pub mod dispatch_plugin;
pub mod planes;
pub mod plugin_registry;
pub mod session_lifecycle;
pub mod trap_gate;
pub mod uart_terminal;
pub mod vmm;

pub use plugin_registry::PluginRegistration;
pub use session_lifecycle::Session;
