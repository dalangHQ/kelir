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
  { method: "onHeartbeat", ring: RingEvent.Heartbeat },
  { method: "onProcessExit", ring: RingEvent.ProcessExit },
  { method: "onVmInit", ring: RingEvent.Lifecycle, lifecycleTag: "vm-init" },
  { method: "onVmReady", ring: RingEvent.Lifecycle, lifecycleTag: "vm-ready" },
  { method: "onVmShutdown", ring: RingEvent.Lifecycle, lifecycleTag: "vm-shutdown" },
  { method: "onWatchdog", ring: RingEvent.Watchdog },
];
