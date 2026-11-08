// see: cans/02-plugin-model.md#KelirPlugin-trait
// see: cans/02-plugin-model.md#Segregated-ring-traits
pub use kelir_interface::driver::{WatchdogEvent, WatchdogVerdict};
pub use kelir_interface::plugin::PluginError;
pub use kelir_interface::plugin::{
    AnyPlugin, CompositePlugin, ConsoleInterposer, EventBusBinder, ExecutionTier,
    ForeignPluginBridge, FsInodeAttr, FsInterposer, HostInterposer, KelirPlugin, LifecycleCtx,
    LifecycleEvent, LifecycleObservation, LifecycleParticipant, LockBrokerCoordination, MountScope,
    NetInterposer, PANIC_QUARANTINE_ENABLED, ParkedConnectionReset, PipelineBinder, PluginClone,
    SystemTelemetryParticipant, TrapInterposer, VetoEvent, VirtioDevice, VirtioRing,
};
pub use kelir_interface::protocol::{FilterAction, ProcessExit, TrapEvent, Verdict};
