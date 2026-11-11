// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/02-plugin-model.md#Foreign-plugin-proxy

import type { LifecycleContext, TrapContext } from "./context.lifecycle.types.js";
import { EConfig } from "./errors.sdk.js";
import {
  LIFECYCLE_TAGS,
  RingEvent,
  type HookDescriptor,
  type HookStage,
  type LifecycleTag,
  type TrapFilterSnapshot,
} from "./fusion.plugin.js";
import type { Schema } from "./schema.config.js";
import type {
  FilterAction,
  Verdict,
} from "./verdict.protocol.js";
import type {
  HeartbeatEvent,
  ProcessExitEvent,
  TrapInfo,
  WatchdogEvent,
  WatchdogVerdict,
} from "./types.core.js";

export interface KelirPlugin {
  readonly name: string;
  readonly enabled?: boolean;
  readonly config?: unknown;
}

// see: cans/artifacts/code/plugin-trait.rs#MountScope
export interface MountScope {
  readonly devices?: readonly string[];
  readonly binaries?: readonly string[];
  readonly paths?: readonly string[];
}

// see: cans/11-protocol.md#Control-channel
export interface VsockServiceEntry {
  readonly port: number;
  readonly service: string;
  readonly schema: Schema<unknown>;
  readonly handler: (request: unknown) => Promise<unknown> | unknown;
}

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
export interface PluginContext<Ctx> {
  readonly pluginCtx?: Ctx;
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export interface TrapScope<Ctx> {
  onTrap(
    handler: (
      event: TrapInfo,
      c: TrapContext & Ctx,
    ) => Promise<Verdict | null> | Verdict | null,
  ): TrapScope<Ctx>;
  onProcessExit(
    handler: (event: ProcessExitEvent) => Promise<void> | void,
  ): TrapScope<Ctx>;
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export interface FsScope<Ctx> {
  onFsLookup(handler: (path: string) => Verdict): FsScope<Ctx>;
  onFsRead(
    handler: (path: string, buffer: Uint8Array) => Uint8Array,
  ): FsScope<Ctx>;
  onFsWrite(
    handler: (
      path: string,
      buffer: Uint8Array,
      pid: number,
      ctx: TrapContext & Ctx,
    ) => FilterAction,
  ): FsScope<Ctx>;
  onFsLock(
    handler: (
      path: string,
      flockCmd: number,
      flockType: number,
      pid: number,
    ) => Verdict,
  ): FsScope<Ctx>;
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export interface NetScope<Ctx> {
  onNetOutbound(handler: (frame: Uint8Array) => FilterAction): NetScope<Ctx>;
  onNetInbound(handler: (frame: Uint8Array) => FilterAction): NetScope<Ctx>;
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export interface ConsoleScope<Ctx> {
  onConsoleInput(
    handler: (
      port: number,
      bytes: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): ConsoleScope<Ctx>;
  onConsoleOutput(
    handler: (
      port: number,
      chunk: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): ConsoleScope<Ctx>;
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export interface ExecScope<Ctx> {
  onExecStdout(
    handler: (
      pid: number,
      execId: number,
      chunk: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): ExecScope<Ctx>;
  onExecStderr(
    handler: (
      pid: number,
      execId: number,
      chunk: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): ExecScope<Ctx>;
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export interface HostScope<Ctx> {
  onHostLsmExec(handler: (path: string, pid: number) => Verdict): HostScope<Ctx>;
  onHostPressure(handler: (level: number) => Verdict): HostScope<Ctx>;
  onHostKeyringAccess(handler: (key: string) => Verdict): HostScope<Ctx>;
  onHostPreempt(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): HostScope<Ctx>;
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export interface LifecycleScope<Ctx> {
  onVmInit(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): LifecycleScope<Ctx>;
  onVmReady(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): LifecycleScope<Ctx>;
  onVmShutdown(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): LifecycleScope<Ctx>;
  onPreCommit(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): LifecycleScope<Ctx>;
  onPostCommit(
    handler: (
      ctx: LifecycleContext,
      committed: boolean,
    ) => Promise<void> | void,
  ): LifecycleScope<Ctx>;
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export interface TelemetryScope<Ctx> {
  onMemoryPressure(
    handler: (
      availableMb: number,
      totalMb: number,
    ) => Promise<void> | void,
  ): TelemetryScope<Ctx>;
  onHeartbeat(
    handler: (event: HeartbeatEvent) => Promise<void> | void,
  ): TelemetryScope<Ctx>;
  onWatchdog(
    handler: (
      event: WatchdogEvent,
    ) => Promise<WatchdogVerdict> | WatchdogVerdict,
  ): TelemetryScope<Ctx>;
}

function unpack(event: unknown): readonly unknown[] {
  return Array.isArray(event) ? (event as readonly unknown[]) : [event];
}

// see: cans/02-plugin-model.md#Segregated-ring-traits
const KNOWN_RINGS: ReadonlySet<string> = new Set<string>([
  ...Object.values(RingEvent),
  "fs-lock",
  "host-lsm-exec",
  "host-pressure",
  "host-keyring-access",
  "host-preempt",
  "memory-pressure",
]);

// see: cans/artifacts/code/plugin-trait.rs#MountScope
function validateScope(scope: MountScope, name: string): void {
  for (const key of Object.keys(scope)) {
    if (key !== "devices" && key !== "binaries" && key !== "paths") {
      throw new EConfig(`${name}: scope has unknown key ${key}`);
    }
  }
  for (const field of ["devices", "binaries", "paths"] as const) {
    const value = scope[field];
    if (
      value !== undefined &&
      (!Array.isArray(value) ||
        value.some((entry) => typeof entry !== "string"))
    ) {
      throw new EConfig(
        `${name}: scope ${field} must be an array of strings`,
      );
    }
  }
}

export class HookRegistry<Ctx = unknown> {
  private readonly pluginName?: string;
  readonly hooks: HookDescriptor[] = [];
  private nextOrdinal = 0;
  private scopeValue?: MountScope;
  readonly requiredDeps: string[] = [];
  readonly deriveFns: Array<(c: unknown) => unknown> = [];
  readonly contextFns: Array<(c: unknown) => unknown> = [];
  readonly vsockEntries: VsockServiceEntry[] = [];
  readonly hostOnlyCalls: string[] = [];

  constructor(pluginName?: string) {
    this.pluginName = pluginName;
  }

  get owner(): string {
    return this.pluginName ?? "host";
  }

  getScope(): MountScope | undefined {
    return this.scopeValue;
  }

  setScope(scope: MountScope): void {
    this.scopeValue = scope;
  }

  // see: cans/02-plugin-model.md#Foreign-plugin-proxy
  addHook(hook: {
    ring: RingEvent;
    stage: HookStage;
    run: (event: unknown) => unknown;
    filter?: TrapFilterSnapshot;
    lifecycleTag?: LifecycleTag;
    customActionName?: string;
    vsockKind?: number;
  }): void {
    this.hooks.push({
      ring: hook.ring,
      stage: hook.stage,
      pluginName: this.pluginName,
      ordinal: this.nextOrdinal,
      filter: hook.filter,
      lifecycleTag: hook.lifecycleTag,
      customActionName: hook.customActionName,
      vsockKind: hook.vsockKind,
      run: hook.run,
    });
    this.nextOrdinal += 1;
  }

  adoptHook(hook: HookDescriptor): void {
    this.hooks.push({ ...hook, ordinal: this.nextOrdinal });
    this.nextOrdinal += 1;
  }

  recordHostOnly(verb: string): void {
    if (!this.hostOnlyCalls.includes(verb)) {
      this.hostOnlyCalls.push(verb);
    }
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  validate(): void {
    const owner = this.owner;
    const hostOnly = this.hostOnlyCalls[0];
    if (hostOnly !== undefined) {
      throw new EConfig(`plugin ${owner} called host-only verb ${hostOnly}`);
    }
    for (const hook of this.hooks) {
      if (!KNOWN_RINGS.has(hook.ring)) {
        throw new EConfig(
          `${owner}: registers unknown ring ${String(hook.ring)}`,
        );
      }
      if (
        hook.ring === RingEvent.Lifecycle &&
        (hook.lifecycleTag === undefined ||
          !LIFECYCLE_TAGS.includes(hook.lifecycleTag))
      ) {
        throw new EConfig(
          `${owner}: lifecycle hook on ordinal ${hook.ordinal} declares unknown lifecycle tag ${String(hook.lifecycleTag)}`,
        );
      }
    }
    for (const entry of this.vsockEntries) {
      if (!Number.isInteger(entry.port) || entry.port < 1 || entry.port > 65535) {
        throw new EConfig(
          `${owner}: vsock service ${entry.service} has invalid port ${String(entry.port)}`,
        );
      }
      if (entry.service.length === 0) {
        throw new EConfig(
          `${owner}: vsock service name must not be empty`,
        );
      }
    }
    if (this.scopeValue !== undefined) {
      validateScope(this.scopeValue, owner);
    }
  }

  clone(): HookRegistry<Ctx> {
    const copy = new HookRegistry<Ctx>(this.pluginName);
    copy.hooks.push(...this.hooks);
    copy.nextOrdinal = this.nextOrdinal;
    copy.scopeValue = this.scopeValue;
    copy.requiredDeps.push(...this.requiredDeps);
    copy.deriveFns.push(...this.deriveFns);
    copy.contextFns.push(...this.contextFns);
    copy.vsockEntries.push(...this.vsockEntries);
    copy.hostOnlyCalls.push(...this.hostOnlyCalls);
    return copy;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  trap(
    fn: (scope: TrapScope<Ctx>) => TrapScope<Ctx>,
  ): HookRegistry<Ctx> {
    const registry = this;
    const scope: TrapScope<Ctx> = {
      onTrap(handler) {
        registry.onTrap(handler);
        return scope;
      },
      onProcessExit(handler) {
        registry.onProcessExit(handler);
        return scope;
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  fs(
    fn: (scope: FsScope<Ctx>) => FsScope<Ctx>,
  ): HookRegistry<Ctx> {
    const registry = this;
    const scope: FsScope<Ctx> = {
      onFsLookup(handler) {
        registry.onFsLookup(handler);
        return scope;
      },
      onFsRead(handler) {
        registry.onFsRead(handler);
        return scope;
      },
      onFsWrite(handler) {
        registry.onFsWrite(handler);
        return scope;
      },
      onFsLock(handler) {
        registry.onFsLock(handler);
        return scope;
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  net(
    fn: (scope: NetScope<Ctx>) => NetScope<Ctx>,
  ): HookRegistry<Ctx> {
    const registry = this;
    const scope: NetScope<Ctx> = {
      onNetOutbound(handler) {
        registry.onNetOutbound(handler);
        return scope;
      },
      onNetInbound(handler) {
        registry.onNetInbound(handler);
        return scope;
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  console(
    fn: (scope: ConsoleScope<Ctx>) => ConsoleScope<Ctx>,
  ): HookRegistry<Ctx> {
    const registry = this;
    const scope: ConsoleScope<Ctx> = {
      onConsoleInput(handler) {
        registry.onConsoleInput(handler);
        return scope;
      },
      onConsoleOutput(handler) {
        registry.onConsoleOutput(handler);
        return scope;
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  exec(
    fn: (scope: ExecScope<Ctx>) => ExecScope<Ctx>,
  ): HookRegistry<Ctx> {
    const registry = this;
    const scope: ExecScope<Ctx> = {
      onExecStdout(handler) {
        registry.onExecStdout(handler);
        return scope;
      },
      onExecStderr(handler) {
        registry.onExecStderr(handler);
        return scope;
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  host(
    fn: (scope: HostScope<Ctx>) => HostScope<Ctx>,
  ): HookRegistry<Ctx> {
    const registry = this;
    const scope: HostScope<Ctx> = {
      onHostLsmExec(handler) {
        registry.onHostLsmExec(handler);
        return scope;
      },
      onHostPressure(handler) {
        registry.onHostPressure(handler);
        return scope;
      },
      onHostKeyringAccess(handler) {
        registry.onHostKeyringAccess(handler);
        return scope;
      },
      onHostPreempt(handler) {
        registry.onHostPreempt(handler);
        return scope;
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  lifecycle(
    fn: (scope: LifecycleScope<Ctx>) => LifecycleScope<Ctx>,
  ): HookRegistry<Ctx> {
    const registry = this;
    const scope: LifecycleScope<Ctx> = {
      onVmInit(handler) {
        registry.onVmInit(handler);
        return scope;
      },
      onVmReady(handler) {
        registry.onVmReady(handler);
        return scope;
      },
      onVmShutdown(handler) {
        registry.onVmShutdown(handler);
        return scope;
      },
      onPreCommit(handler) {
        registry.onPreCommit(handler);
        return scope;
      },
      onPostCommit(handler) {
        registry.onPostCommit(handler);
        return scope;
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  telemetry(
    fn: (scope: TelemetryScope<Ctx>) => TelemetryScope<Ctx>,
  ): HookRegistry<Ctx> {
    const registry = this;
    const scope: TelemetryScope<Ctx> = {
      onMemoryPressure(handler) {
        registry.onMemoryPressure(handler);
        return scope;
      },
      onHeartbeat(handler) {
        registry.onHeartbeat(handler);
        return scope;
      },
      onWatchdog(handler) {
        registry.onWatchdog(handler);
        return scope;
      },
    };
    fn(scope);
    return this;
  }

  onNetOutbound(
    handler: (frame: Uint8Array) => FilterAction,
  ): void {
    this.addHook({
      ring: RingEvent.NetOutbound,
      stage: "single",
      run: (event) => handler(unpack(event)[0] as Uint8Array),
    });
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onNetInbound(
    handler: (frame: Uint8Array) => FilterAction,
  ): void {
    this.addHook({
      ring: RingEvent.NetInbound,
      stage: "single",
      run: (event) => handler(unpack(event)[0] as Uint8Array),
    });
  }

  onFsLookup(
    handler: (path: string) => Verdict,
  ): void {
    this.addHook({
      ring: RingEvent.FsLookup,
      stage: "single",
      run: (event) => handler(unpack(event)[0] as string),
    });
  }

  onFsRead(
    handler: (path: string, buffer: Uint8Array) => Uint8Array,
  ): void {
    this.addHook({
      ring: RingEvent.FsRead,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(args[0] as string, args[1] as Uint8Array);
      },
    });
  }

  onFsWrite(
    handler: (
      path: string,
      buffer: Uint8Array,
      pid: number,
      ctx: TrapContext & Ctx,
    ) => FilterAction,
  ): void {
    this.addHook({
      ring: RingEvent.FsWrite,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(
          args[0] as string,
          args[1] as Uint8Array,
          args[2] as number,
          args[3] as TrapContext & Ctx,
        );
      },
    });
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onFsLock(
    handler: (
      path: string,
      flockCmd: number,
      flockType: number,
      pid: number,
    ) => Verdict,
  ): void {
    this.addHook({
      ring: "fs-lock" as RingEvent,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(
          args[0] as string,
          args[1] as number,
          args[2] as number,
          args[3] as number,
        );
      },
    });
  }

  onConsoleInput(
    handler: (
      port: number,
      bytes: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): void {
    this.addHook({
      ring: RingEvent.ConsoleInput,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(args[0] as number, args[1] as Uint8Array);
      },
    });
  }

  onConsoleOutput(
    handler: (
      port: number,
      chunk: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): void {
    this.addHook({
      ring: RingEvent.ConsoleOutput,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(args[0] as number, args[1] as Uint8Array);
      },
    });
  }

  // see: cans/02-plugin-model.md#KelirPlugin-trait
  onExecStdout(
    handler: (
      pid: number,
      execId: number,
      chunk: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): void {
    this.addHook({
      ring: RingEvent.ExecStdout,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(
          args[0] as number,
          args[1] as number,
          args[2] as Uint8Array,
        );
      },
    });
  }

  // see: cans/02-plugin-model.md#KelirPlugin-trait
  onExecStderr(
    handler: (
      pid: number,
      execId: number,
      chunk: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): void {
    this.addHook({
      ring: RingEvent.ExecStderr,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(
          args[0] as number,
          args[1] as number,
          args[2] as Uint8Array,
        );
      },
    });
  }

  onHeartbeat(
    handler: (event: HeartbeatEvent) => Promise<void> | void,
  ): void {
    this.addHook({
      ring: RingEvent.Heartbeat,
      stage: "single",
      run: (event) => handler(unpack(event)[0] as HeartbeatEvent),
    });
  }

  onProcessExit(
    handler: (event: ProcessExitEvent) => Promise<void> | void,
  ): void {
    this.addHook({
      ring: RingEvent.ProcessExit,
      stage: "single",
      run: (event) => handler(unpack(event)[0] as ProcessExitEvent),
    });
  }

  onTrap(
    handler: (
      event: TrapInfo,
      c: TrapContext & Ctx,
    ) => Promise<Verdict | null> | Verdict | null,
  ): void {
    this.addHook({
      ring: RingEvent.Trap,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(args[0] as TrapInfo, args[1] as TrapContext & Ctx);
      },
    });
  }

  onWatchdog(
    handler: (
      event: WatchdogEvent,
    ) => Promise<WatchdogVerdict> | WatchdogVerdict,
  ): void {
    this.addHook({
      ring: RingEvent.Watchdog,
      stage: "single",
      run: (event) => handler(unpack(event)[0] as WatchdogEvent),
    });
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onHostLsmExec(
    handler: (path: string, pid: number) => Verdict,
  ): void {
    this.addHook({
      ring: "host-lsm-exec" as RingEvent,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(args[0] as string, args[1] as number);
      },
    });
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onHostPressure(
    handler: (level: number) => Verdict,
  ): void {
    this.addHook({
      ring: "host-pressure" as RingEvent,
      stage: "single",
      run: (event) => handler(unpack(event)[0] as number),
    });
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onHostKeyringAccess(
    handler: (key: string) => Verdict,
  ): void {
    this.addHook({
      ring: "host-keyring-access" as RingEvent,
      stage: "single",
      run: (event) => handler(unpack(event)[0] as string),
    });
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onVmInit(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): void {
    this.addHook({
      ring: RingEvent.Lifecycle,
      stage: "single",
      lifecycleTag: "vm-init",
      run: (event) => handler(unpack(event)[0] as LifecycleContext),
    });
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onVmReady(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): void {
    this.addHook({
      ring: RingEvent.Lifecycle,
      stage: "single",
      lifecycleTag: "vm-ready",
      run: (event) => handler(unpack(event)[0] as LifecycleContext),
    });
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onVmShutdown(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): void {
    this.addHook({
      ring: RingEvent.Lifecycle,
      stage: "single",
      lifecycleTag: "vm-shutdown",
      run: (event) => handler(unpack(event)[0] as LifecycleContext),
    });
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onPreCommit(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): void {
    this.addHook({
      ring: RingEvent.Lifecycle,
      stage: "single",
      lifecycleTag: "pre-commit",
      run: (event) => handler(unpack(event)[0] as LifecycleContext),
    });
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onPostCommit(
    handler: (
      ctx: LifecycleContext,
      committed: boolean,
    ) => Promise<void> | void,
  ): void {
    this.addHook({
      ring: RingEvent.Lifecycle,
      stage: "single",
      lifecycleTag: "post-commit",
      run: (event) => {
        const args = unpack(event);
        return handler(args[0] as LifecycleContext, args[1] as boolean);
      },
    });
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onHostPreempt(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): void {
    this.addHook({
      ring: "host-preempt" as RingEvent,
      stage: "single",
      run: (event) => handler(unpack(event)[0] as LifecycleContext),
    });
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onMemoryPressure(
    handler: (
      availableMb: number,
      totalMb: number,
    ) => Promise<void> | void,
  ): void {
    this.addHook({
      ring: "memory-pressure" as RingEvent,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(args[0] as number, args[1] as number);
      },
    });
  }
}
