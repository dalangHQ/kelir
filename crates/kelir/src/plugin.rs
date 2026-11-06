// see: cans/02-plugin-model.md#KelirPlugin-trait
// see: cans/02-plugin-model.md#Segregated-ring-traits
pub use kelir_interface::driver::{WatchdogEvent, WatchdogVerdict};
pub use kelir_interface::plugin::PluginError;
pub use kelir_interface::plugin::{
    AnyPlugin, CompositePlugin, ConsoleInterposer, EventBusBinder, ExecutionTier,
    ForeignPluginBridge, FsInodeAttr, FsInterposer, HandlerCtx, HostInterposer, KelirPlugin,
    LifecycleCtx, LifecycleEvent, LifecycleObservation, LifecycleParticipant, MountScope,
    NetInterposer, PANIC_QUARANTINE_ENABLED, PipelineBinder, PluginClone,
    SystemTelemetryParticipant, TrapInterposer, VetoEvent, VirtioDevice, VirtioRing,
};
pub use kelir_interface::protocol::{FilterAction, ProcessExit, TrapEvent, Verdict};
pub use kelir_interface::service_registry::ServiceRegistry;
