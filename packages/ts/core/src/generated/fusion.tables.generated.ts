// see: cans/02-plugin-model.md#Segregated-ring-traits
// see: cans/02-plugin-model.md#Hook-context-restriction
// see: cans/02-plugin-model.md#Foreign-plugin-proxy

export enum RingEvent {
  Trap = "trap",
  TrapDeny = "trap-deny",
  ProcessExit = "process-exit",
  FsLookup = "fs-lookup",
  FsRead = "fs-read",
  FsWrite = "fs-write",
  NetOutbound = "net-outbound",
  NetInbound = "net-inbound",
  ConsoleInput = "console-input",
  ConsoleOutput = "console-output",
  ExecStdout = "exec-stdout",
  ExecStderr = "exec-stderr",
  Heartbeat = "heartbeat",
  Lifecycle = "lifecycle",
  Watchdog = "watchdog",
  CustomAction = "custom-action",
  VsockService = "vsock-service",
  FsLock = "fs-lock",
  HostLsmExec = "host-lsm-exec",
  HostPressure = "host-pressure",
  HostKeyringAccess = "host-keyring-access",
  HostPreempt = "host-preempt",
  MemoryPressure = "memory-pressure",
}

export type HookStage = "pre" | "post" | "single";

export type LifecycleTag =
  | "vm-init"
  | "vm-ready"
  | "vm-shutdown"
  | "trap-deny"
  | "pre-commit"
  | "post-commit";

export const LIFECYCLE_TAGS: readonly string[] = [
  "vm-init",
  "vm-ready",
  "vm-shutdown",
  "trap-deny",
  "pre-commit",
  "post-commit",
];

export interface HookRingBinding {
  readonly method: string;
  readonly ring: RingEvent;
  readonly lifecycleTag?: LifecycleTag;
}

export const HOOK_RINGS: readonly HookRingBinding[] = [
  { method: "onTrap", ring: RingEvent.Trap },
  { method: "onFsLookup", ring: RingEvent.FsLookup },
  { method: "onFsRead", ring: RingEvent.FsRead },
  { method: "onFsWrite", ring: RingEvent.FsWrite },
  { method: "onNetOutbound", ring: RingEvent.NetOutbound },
  { method: "onConsoleInput", ring: RingEvent.ConsoleInput },
  { method: "onConsoleOutput", ring: RingEvent.ConsoleOutput },
  { method: "onExecStdout", ring: RingEvent.ExecStdout },
  { method: "onExecStderr", ring: RingEvent.ExecStderr },
  { method: "onHeartbeat", ring: RingEvent.Heartbeat },
  { method: "onProcessExit", ring: RingEvent.ProcessExit },
  { method: "onVmInit", ring: RingEvent.Lifecycle, lifecycleTag: "vm-init" },
  { method: "onVmReady", ring: RingEvent.Lifecycle, lifecycleTag: "vm-ready" },
  { method: "onVmShutdown", ring: RingEvent.Lifecycle, lifecycleTag: "vm-shutdown" },
  { method: "onWatchdog", ring: RingEvent.Watchdog },
  { method: "onFsLock", ring: RingEvent.FsLock },
  { method: "onNetInbound", ring: RingEvent.NetInbound },
  { method: "onHostLsmExec", ring: RingEvent.HostLsmExec },
  { method: "onHostPressure", ring: RingEvent.HostPressure },
  { method: "onHostKeyringAccess", ring: RingEvent.HostKeyringAccess },
  { method: "onPreCommit", ring: RingEvent.Lifecycle, lifecycleTag: "pre-commit" },
  { method: "onPostCommit", ring: RingEvent.Lifecycle, lifecycleTag: "post-commit" },
  { method: "onHostPreempt", ring: RingEvent.HostPreempt },
  { method: "onMemoryPressure", ring: RingEvent.MemoryPressure },
];

export interface PluginBuilderVerb {
  readonly verb: string;
  readonly ring: RingEvent;
  readonly lifecycleTag?: LifecycleTag;
}

export const PLUGIN_BUILDER_VERBS: readonly PluginBuilderVerb[] = [
  { verb: "onTrap", ring: RingEvent.Trap },
  { verb: "onFsLookup", ring: RingEvent.FsLookup },
  { verb: "onFsRead", ring: RingEvent.FsRead },
  { verb: "onFsWrite", ring: RingEvent.FsWrite },
  { verb: "onNetOutbound", ring: RingEvent.NetOutbound },
  { verb: "onConsoleInput", ring: RingEvent.ConsoleInput },
  { verb: "onConsoleOutput", ring: RingEvent.ConsoleOutput },
  { verb: "onExecStdout", ring: RingEvent.ExecStdout },
  { verb: "onExecStderr", ring: RingEvent.ExecStderr },
  { verb: "onHeartbeat", ring: RingEvent.Heartbeat },
  { verb: "onProcessExit", ring: RingEvent.ProcessExit },
  { verb: "onVmInit", ring: RingEvent.Lifecycle, lifecycleTag: "vm-init" },
  { verb: "onVmReady", ring: RingEvent.Lifecycle, lifecycleTag: "vm-ready" },
  { verb: "onVmShutdown", ring: RingEvent.Lifecycle, lifecycleTag: "vm-shutdown" },
  { verb: "onWatchdog", ring: RingEvent.Watchdog },
  { verb: "onFsLock", ring: RingEvent.FsLock },
  { verb: "onNetInbound", ring: RingEvent.NetInbound },
  { verb: "onHostLsmExec", ring: RingEvent.HostLsmExec },
  { verb: "onHostPressure", ring: RingEvent.HostPressure },
  { verb: "onHostKeyringAccess", ring: RingEvent.HostKeyringAccess },
  { verb: "onPreCommit", ring: RingEvent.Lifecycle, lifecycleTag: "pre-commit" },
  { verb: "onPostCommit", ring: RingEvent.Lifecycle, lifecycleTag: "post-commit" },
  { verb: "onHostPreempt", ring: RingEvent.HostPreempt },
  { verb: "onMemoryPressure", ring: RingEvent.MemoryPressure },
];
