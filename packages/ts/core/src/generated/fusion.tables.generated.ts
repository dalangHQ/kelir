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
];

export interface PluginBuilderVerb {
  readonly verb: string;
  readonly ring: RingEvent;
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
  { verb: "onVmInit", ring: RingEvent.Lifecycle },
  { verb: "onVmReady", ring: RingEvent.Lifecycle },
  { verb: "onVmShutdown", ring: RingEvent.Lifecycle },
  { verb: "onWatchdog", ring: RingEvent.Watchdog },
];
