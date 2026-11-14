// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/02-plugin-model.md#Foreign-plugin-proxy

import type { LifecycleContext, TrapContext } from "./context.lifecycle.types.js";
import { EConfig } from "./errors.sdk.js";
import type { Buffer } from "node:buffer";
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
export interface TrapScope<Ctx, Owner = unknown> {
  onTrap(
    handler: (
      event: TrapInfo,
      c: TrapContext & Ctx,
    ) => Promise<Verdict> | Verdict,
  ): TrapScope<Ctx, Owner>;
  onProcessExit(
    handler: (event: ProcessExitEvent) => Promise<void> | void,
  ): TrapScope<Ctx, Owner>;
  end(): Owner;
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export interface FsScope<Ctx, Owner = unknown> {
  onFsLookup(handler: (path: string) => Verdict): FsScope<Ctx, Owner>;
  onFsRead(
    handler: (path: string, buffer: Uint8Array) => Uint8Array,
  ): FsScope<Ctx, Owner>;
  onFsWrite(
    handler: (
      path: string,
      buffer: Buffer,
      pid: number,
      ctx: TrapContext & Ctx,
    ) => FilterAction,
  ): FsScope<Ctx, Owner>;
  onFsLock(
    handler: (
      path: string,
      flockCmd: number,
      flockType: number,
      pid: number,
    ) => Verdict,
  ): FsScope<Ctx, Owner>;
  end(): Owner;
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export interface NetScope<Ctx, Owner = unknown> {
  onNetOutbound(handler: (frame: Uint8Array) => FilterAction): NetScope<Ctx, Owner>;
  onNetInbound(handler: (frame: Uint8Array) => FilterAction): NetScope<Ctx, Owner>;
  end(): Owner;
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export interface ConsoleScope<Ctx, Owner = unknown> {
  onConsoleInput(
    handler: (
      port: number,
      bytes: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): ConsoleScope<Ctx, Owner>;
  onConsoleOutput(
    handler: (
      port: number,
      chunk: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): ConsoleScope<Ctx, Owner>;
  end(): Owner;
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export interface ExecScope<Ctx, Owner = unknown> {
  onExecStdout(
    handler: (
      pid: number,
      execId: number,
      chunk: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): ExecScope<Ctx, Owner>;
  onExecStderr(
    handler: (
      pid: number,
      execId: number,
      chunk: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): ExecScope<Ctx, Owner>;
  end(): Owner;
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export interface HostScope<Ctx, Owner = unknown> {
  onHostLsmExec(handler: (path: string, pid: number) => Verdict): HostScope<Ctx, Owner>;
  onHostPressure(handler: (level: number) => Verdict): HostScope<Ctx, Owner>;
  onHostKeyringAccess(handler: (key: string) => Verdict): HostScope<Ctx, Owner>;
  onHostPreempt(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): HostScope<Ctx, Owner>;
  end(): Owner;
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export interface LifecycleScope<Ctx, Owner = unknown> {
  onVmInit(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): LifecycleScope<Ctx, Owner>;
  onVmReady(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): LifecycleScope<Ctx, Owner>;
  onVmShutdown(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): LifecycleScope<Ctx, Owner>;
  onPreCommit(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): LifecycleScope<Ctx, Owner>;
  onPostCommit(
    handler: (
      ctx: LifecycleContext,
      committed: boolean,
    ) => Promise<void> | void,
  ): LifecycleScope<Ctx, Owner>;
  end(): Owner;
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export interface TelemetryScope<Ctx, Owner = unknown> {
  onMemoryPressure(
    handler: (
      availableMb: number,
      totalMb: number,
    ) => Promise<void> | void,
  ): TelemetryScope<Ctx, Owner>;
  onHeartbeat(
    handler: (event: HeartbeatEvent) => Promise<void> | void,
  ): TelemetryScope<Ctx, Owner>;
  onWatchdog(
    handler: (
      event: WatchdogEvent,
    ) => Promise<WatchdogVerdict> | WatchdogVerdict,
  ): TelemetryScope<Ctx, Owner>;
  end(): Owner;
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

  constructor(pluginName?: string) {
    this.pluginName = pluginName;
  }

  get owner(): string {
    return this.pluginName ?? "host";
  }

  getScope(): MountScope | undefined {
    return this.scopeValue;
  }

  setScope(scope: MountScope): this {
    this.scopeValue = scope;
    return this;
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
  }): this {
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
    return this;
  }

  adoptHook(hook: HookDescriptor): this {
    this.hooks.push({ ...hook, ordinal: this.nextOrdinal });
    this.nextOrdinal += 1;
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  validate(): this {
    const owner = this.owner;
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
    return this;
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
    return copy;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  trap<Owner>(
    fn: (scope: TrapScope<Ctx, Owner>) => TrapScope<Ctx, Owner> | void,
    endTarget: () => Owner,
  ): HookRegistry<Ctx> {
    const registry = this;
    const scope: TrapScope<Ctx, Owner> = {
      onTrap(handler) {
        registry.onTrap(handler);
        return scope;
      },
      onProcessExit(handler) {
        registry.onProcessExit(handler);
        return scope;
      },
      end() {
        return endTarget();
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  fs<Owner>(
    fn: (scope: FsScope<Ctx, Owner>) => FsScope<Ctx, Owner> | void,
    endTarget: () => Owner,
  ): HookRegistry<Ctx> {
    const registry = this;
    const scope: FsScope<Ctx, Owner> = {
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
      end() {
        return endTarget();
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  net<Owner>(
    fn: (scope: NetScope<Ctx, Owner>) => NetScope<Ctx, Owner> | void,
    endTarget: () => Owner,
  ): HookRegistry<Ctx> {
    const registry = this;
    const scope: NetScope<Ctx, Owner> = {
      onNetOutbound(handler) {
        registry.onNetOutbound(handler);
        return scope;
      },
      onNetInbound(handler) {
        registry.onNetInbound(handler);
        return scope;
      },
      end() {
        return endTarget();
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  console<Owner>(
    fn: (scope: ConsoleScope<Ctx, Owner>) => ConsoleScope<Ctx, Owner> | void,
    endTarget: () => Owner,
  ): HookRegistry<Ctx> {
    const registry = this;
    const scope: ConsoleScope<Ctx, Owner> = {
      onConsoleInput(handler) {
        registry.onConsoleInput(handler);
        return scope;
      },
      onConsoleOutput(handler) {
        registry.onConsoleOutput(handler);
        return scope;
      },
      end() {
        return endTarget();
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  exec<Owner>(
    fn: (scope: ExecScope<Ctx, Owner>) => ExecScope<Ctx, Owner> | void,
    endTarget: () => Owner,
  ): HookRegistry<Ctx> {
    const registry = this;
    const scope: ExecScope<Ctx, Owner> = {
      onExecStdout(handler) {
        registry.onExecStdout(handler);
        return scope;
      },
      onExecStderr(handler) {
        registry.onExecStderr(handler);
        return scope;
      },
      end() {
        return endTarget();
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  host<Owner>(
    fn: (scope: HostScope<Ctx, Owner>) => HostScope<Ctx, Owner> | void,
    endTarget: () => Owner,
  ): HookRegistry<Ctx> {
    const registry = this;
    const scope: HostScope<Ctx, Owner> = {
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
      end() {
        return endTarget();
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  lifecycle<Owner>(
    fn: (scope: LifecycleScope<Ctx, Owner>) => LifecycleScope<Ctx, Owner> | void,
    endTarget: () => Owner,
  ): HookRegistry<Ctx> {
    const registry = this;
    const scope: LifecycleScope<Ctx, Owner> = {
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
      end() {
        return endTarget();
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  telemetry<Owner>(
    fn: (scope: TelemetryScope<Ctx, Owner>) => TelemetryScope<Ctx, Owner> | void,
    endTarget: () => Owner,
  ): HookRegistry<Ctx> {
    const registry = this;
    const scope: TelemetryScope<Ctx, Owner> = {
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
      end() {
        return endTarget();
      },
    };
    fn(scope);
    return this;
  }

  onNetOutbound(
    handler: (frame: Uint8Array) => FilterAction,
  ): this {
    this.addHook({
      ring: RingEvent.NetOutbound,
      stage: "single",
      run: (event) => handler(unpack(event)[0] as Uint8Array),
    });
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onNetInbound(
    handler: (frame: Uint8Array) => FilterAction,
  ): this {
    this.addHook({
      ring: RingEvent.NetInbound,
      stage: "single",
      run: (event) => handler(unpack(event)[0] as Uint8Array),
    });
    return this;
  }

  onFsLookup(
    handler: (path: string) => Verdict,
  ): this {
    this.addHook({
      ring: RingEvent.FsLookup,
      stage: "single",
      run: (event) => handler(unpack(event)[0] as string),
    });
    return this;
  }

  onFsRead(
    handler: (path: string, buffer: Uint8Array) => Uint8Array,
  ): this {
    this.addHook({
      ring: RingEvent.FsRead,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(args[0] as string, args[1] as Uint8Array);
      },
    });
    return this;
  }

  onFsWrite(
    handler: (
      path: string,
      buffer: Buffer,
      pid: number,
      ctx: TrapContext & Ctx,
    ) => FilterAction,
  ): this {
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
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onFsLock(
    handler: (
      path: string,
      flockCmd: number,
      flockType: number,
      pid: number,
    ) => Verdict,
  ): this {
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
    return this;
  }

  onConsoleInput(
    handler: (
      port: number,
      bytes: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): this {
    this.addHook({
      ring: RingEvent.ConsoleInput,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(args[0] as number, args[1] as Uint8Array);
      },
    });
    return this;
  }

  onConsoleOutput(
    handler: (
      port: number,
      chunk: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): this {
    this.addHook({
      ring: RingEvent.ConsoleOutput,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(args[0] as number, args[1] as Uint8Array);
      },
    });
    return this;
  }

  // see: cans/02-plugin-model.md#KelirPlugin-trait
  onExecStdout(
    handler: (
      pid: number,
      execId: number,
      chunk: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): this {
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
    return this;
  }

  // see: cans/02-plugin-model.md#KelirPlugin-trait
  onExecStderr(
    handler: (
      pid: number,
      execId: number,
      chunk: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): this {
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
    return this;
  }

  onHeartbeat(
    handler: (event: HeartbeatEvent) => Promise<void> | void,
  ): this {
    this.addHook({
      ring: RingEvent.Heartbeat,
      stage: "single",
      run: (event) => handler(unpack(event)[0] as HeartbeatEvent),
    });
    return this;
  }

  onProcessExit(
    handler: (event: ProcessExitEvent) => Promise<void> | void,
  ): this {
    this.addHook({
      ring: RingEvent.ProcessExit,
      stage: "single",
      run: (event) => handler(unpack(event)[0] as ProcessExitEvent),
    });
    return this;
  }

  onTrap(
    handler: (
      event: TrapInfo,
      c: TrapContext & Ctx,
    ) => Promise<Verdict> | Verdict,
  ): this {
    this.addHook({
      ring: RingEvent.Trap,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(args[0] as TrapInfo, args[1] as TrapContext & Ctx);
      },
    });
    return this;
  }

  onWatchdog(
    handler: (
      event: WatchdogEvent,
    ) => Promise<WatchdogVerdict> | WatchdogVerdict,
  ): this {
    this.addHook({
      ring: RingEvent.Watchdog,
      stage: "single",
      run: (event) => handler(unpack(event)[0] as WatchdogEvent),
    });
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onHostLsmExec(
    handler: (path: string, pid: number) => Verdict,
  ): this {
    this.addHook({
      ring: "host-lsm-exec" as RingEvent,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(args[0] as string, args[1] as number);
      },
    });
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onHostPressure(
    handler: (level: number) => Verdict,
  ): this {
    this.addHook({
      ring: "host-pressure" as RingEvent,
      stage: "single",
      run: (event) => handler(unpack(event)[0] as number),
    });
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onHostKeyringAccess(
    handler: (key: string) => Verdict,
  ): this {
    this.addHook({
      ring: "host-keyring-access" as RingEvent,
      stage: "single",
      run: (event) => handler(unpack(event)[0] as string),
    });
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onVmInit(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): this {
    this.addHook({
      ring: RingEvent.Lifecycle,
      stage: "single",
      lifecycleTag: "vm-init",
      run: (event) => handler(unpack(event)[0] as LifecycleContext),
    });
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onVmReady(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): this {
    this.addHook({
      ring: RingEvent.Lifecycle,
      stage: "single",
      lifecycleTag: "vm-ready",
      run: (event) => handler(unpack(event)[0] as LifecycleContext),
    });
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onVmShutdown(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): this {
    this.addHook({
      ring: RingEvent.Lifecycle,
      stage: "single",
      lifecycleTag: "vm-shutdown",
      run: (event) => handler(unpack(event)[0] as LifecycleContext),
    });
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onPreCommit(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): this {
    this.addHook({
      ring: RingEvent.Lifecycle,
      stage: "single",
      lifecycleTag: "pre-commit",
      run: (event) => handler(unpack(event)[0] as LifecycleContext),
    });
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onPostCommit(
    handler: (
      ctx: LifecycleContext,
      committed: boolean,
    ) => Promise<void> | void,
  ): this {
    this.addHook({
      ring: RingEvent.Lifecycle,
      stage: "single",
      lifecycleTag: "post-commit",
      run: (event) => {
        const args = unpack(event);
        return handler(args[0] as LifecycleContext, args[1] as boolean);
      },
    });
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onHostPreempt(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): this {
    this.addHook({
      ring: "host-preempt" as RingEvent,
      stage: "single",
      run: (event) => handler(unpack(event)[0] as LifecycleContext),
    });
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onMemoryPressure(
    handler: (
      availableMb: number,
      totalMb: number,
    ) => Promise<void> | void,
  ): this {
    this.addHook({
      ring: "memory-pressure" as RingEvent,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(args[0] as number, args[1] as number);
      },
    });
    return this;
  }
}
