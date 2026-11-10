// see: cans/02-plugin-model.md#KelirPlugin-trait
// see: cans/02-plugin-model.md#Segregated-ring-traits
pub use kelir_interface::driver_interface::{WatchdogEvent, WatchdogVerdict};
pub use kelir_interface::plugin_model::PluginError;
pub use kelir_interface::plugin_model::{
    AnyPlugin, CompositePlugin, ConsoleInterposer, EventBusBinder, ExecutionTier,
    ForeignPluginBridge, FsInodeAttr, FsInterposer, HostInterposer, KelirPlugin, LifecycleCtx,
    LifecycleEvent, LifecycleObservation, LifecycleParticipant, LockBrokerCoordination, MountScope,
    NetInterposer, NoDeps, PANIC_QUARANTINE_ENABLED, ParkedConnectionReset, PipelineBinder,
    PluginClone, PluginDeps, SystemTelemetryParticipant, TrapInterposer, VetoEvent, VirtioDevice,
    VirtioRing,
};
pub use kelir_interface::protocol_wire::{FilterAction, ProcessExit, TrapEvent, Verdict};
