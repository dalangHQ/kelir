// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/02-plugin-model.md#Foreign-plugin-proxy

import type { LifecycleContext, TrapContext } from "./context.lifecycle.types.js";
import { EConfig } from "./errors.sdk.js";
import {
  LIFECYCLE_TAGS,
  RingEvent,
  type HookDescriptor,
  type LifecycleTag,
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
export interface PluginVsockService {
  readonly port: number;
  readonly service: string;
  readonly schema: Schema<unknown>;
  readonly handler: (request: unknown) => Promise<unknown> | unknown;
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
export interface BuiltPlugin<
  Name extends string = string,
  Ctx = unknown,
  Deps extends string = never,
> extends KelirPlugin {
  readonly name: Name;
  readonly scope?: MountScope;
  readonly requiredDeps: readonly string[];
  readonly deriveFns: readonly ((c: unknown) => unknown)[];
  readonly contextFns: readonly ((c: unknown) => unknown)[];
  readonly vsockServices: readonly PluginVsockService[];
  hooksForFusion(): readonly HookDescriptor[];
  registerInterceptors(bind: (hook: HookDescriptor) => void): void;
  forkState(): BuiltPlugin<Name, Ctx, Deps>;
}

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
export interface PluginContext<Ctx> {
  readonly pluginCtx?: Ctx;
}

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
export type PluginContextOf<P> =
  P extends BuiltPlugin<infer Name, infer C, any>
    ? (C extends Record<string, unknown>
        ? (string extends keyof C
            ? { readonly [K in Name]: C }
            : (Name extends keyof C ? C : { readonly [K in Name]: C }))
        : { readonly [K in Name]: C })
    : P extends PluginContract<infer Name, infer C, any>
    ? (C extends Record<string, unknown>
        ? (string extends keyof C
            ? { readonly [K in Name]: C }
            : (Name extends keyof C ? C : { readonly [K in Name]: C }))
        : { readonly [K in Name]: C })
    : P extends PluginContext<infer C>
      ? C
      : unknown;

export interface CreatePluginOptions {
  readonly opts?: unknown;
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
      throw new EConfig(`plugin ${name} scope has unknown key ${key}`);
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
        `plugin ${name} scope ${field} must be an array of strings`,
      );
    }
  }
}

export class PluginContract<
  Name extends string = string,
  Ctx = unknown,
  Deps extends string = never,
> {
  readonly name: Name;
  private readonly hooks: HookDescriptor[] = [];
  private nextOrdinal = 0;
  private readonly storedOptions?: Ctx | CreatePluginOptions;
  private scopeValue?: MountScope;
  private forkFn?: () => PluginContract<Name, Ctx, Deps>;
  private built = false;
  private readonly requiredDeps: string[] = [];
  private readonly deriveFns: Array<(c: unknown) => unknown> = [];
  private readonly contextFns: Array<(c: unknown) => unknown> = [];
  private readonly vsockEntries: PluginVsockService[] = [];

  constructor(name: Name, options?: Ctx | CreatePluginOptions) {
    this.name = name;
    this.storedOptions = options;
  }

  private checkOpen(verb: string): void {
    if (this.built) {
      throw new EConfig(
        `plugin ${this.name} is already built: ${verb} after .build()`,
      );
    }
  }

  // see: cans/03-sdk-architecture.md#TypeScript-SDK
  requires<Req extends string | PluginContract<any, any, any> | BuiltPlugin<any, any, any>>(
    name?: string | PluginContract<any, any, any> | BuiltPlugin<any, any, any>,
  ): PluginContract<
    Name,
    Ctx & (Req extends PluginContract<infer N, infer C, any>
      ? (C extends Record<string, unknown> ? (string extends keyof C ? { readonly [K in N]: C } : (N extends keyof C ? C : { readonly [K in N]: C })) : { readonly [K in N]: C })
      : Req extends BuiltPlugin<infer N, infer C, any>
        ? (C extends Record<string, unknown> ? (string extends keyof C ? { readonly [K in N]: C } : (N extends keyof C ? C : { readonly [K in N]: C })) : { readonly [K in N]: C })
        : unknown),
    Deps | (Req extends PluginContract<infer N, any, any> ? N : Req extends BuiltPlugin<infer N, any, any> ? N : Req extends string ? Req : never)
  > {
    this.checkOpen(".requires");
    if (typeof name === "string") {
      if (!this.requiredDeps.includes(name)) {
        this.requiredDeps.push(name);
      }
    } else if (name !== undefined) {
      const depName = (name as { readonly name: string }).name;
      if (!this.requiredDeps.includes(depName)) {
        this.requiredDeps.push(depName);
      }
    }
    return this as unknown as PluginContract<
      Name,
      Ctx & (Req extends PluginContract<infer N, infer C, any>
        ? (C extends Record<string, unknown> ? (string extends keyof C ? { readonly [K in N]: C } : (N extends keyof C ? C : { readonly [K in N]: C })) : { readonly [K in N]: C })
        : Req extends BuiltPlugin<infer N, infer C, any>
          ? (C extends Record<string, unknown> ? (string extends keyof C ? { readonly [K in N]: C } : (N extends keyof C ? C : { readonly [K in N]: C })) : { readonly [K in N]: C })
          : unknown),
      Deps | (Req extends PluginContract<infer N, any, any> ? N : Req extends BuiltPlugin<infer N, any, any> ? N : Req extends string ? Req : never)
    >;
  }

  // see: cans/03-sdk-architecture.md#TypeScript-SDK
  derive<NewCtx>(
    fn: (c: Ctx) => NewCtx,
  ): PluginContract<Name, Ctx & NewCtx, Deps> {
    this.checkOpen(".derive");
    this.deriveFns.push(fn as (c: unknown) => unknown);
    return this as unknown as PluginContract<Name, Ctx & NewCtx, Deps>;
  }

  // see: cans/03-sdk-architecture.md#TypeScript-SDK
  context<NewCtx>(
    fn: (c: Ctx) => NewCtx,
  ): PluginContract<Name, Ctx & NewCtx, Deps> {
    this.checkOpen(".context");
    this.contextFns.push(fn as (c: unknown) => unknown);
    return this as unknown as PluginContract<Name, Ctx & NewCtx, Deps>;
  }

  // see: cans/artifacts/code/plugin-trait.rs#MountScope
  scope(scope: MountScope): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".scope");
    this.scopeValue = scope;
    return this;
  }

  // see: cans/02-plugin-model.md#KelirPlugin-trait
  forkWith(
    fn: () => PluginContract<Name, Ctx, Deps>,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".forkWith");
    this.forkFn = fn;
    return this;
  }

  // see: cans/11-protocol.md#Control-channel
  vsockService<Req, Resp = unknown>(
    port: number,
    service: string,
    schema: Schema<Req>,
    handler: (request: Req) => Promise<Resp> | Resp,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".vsockService");
    this.vsockEntries.push({
      port,
      service,
      schema: schema as Schema<unknown>,
      handler: handler as (request: unknown) => Promise<unknown> | unknown,
    });
    return this;
  }

  // see: cans/03-sdk-architecture.md#TypeScript-SDK
  build(): BuiltPlugin<Name, Ctx, Deps> {
    this.checkOpen(".build");
    for (const hook of this.hooks) {
      if (!KNOWN_RINGS.has(hook.ring)) {
        throw new EConfig(
          `plugin ${this.name} registers unknown ring ${String(hook.ring)}`,
        );
      }
      if (
        hook.ring === RingEvent.Lifecycle &&
        (hook.lifecycleTag === undefined ||
          !LIFECYCLE_TAGS.includes(hook.lifecycleTag))
      ) {
        throw new EConfig(
          `plugin ${this.name} lifecycle hook on ordinal ${hook.ordinal} declares unknown lifecycle tag ${String(hook.lifecycleTag)}`,
        );
      }
    }
    for (const entry of this.vsockEntries) {
      if (!Number.isInteger(entry.port) || entry.port < 1 || entry.port > 65535) {
        throw new EConfig(
          `plugin ${this.name} vsock service ${entry.service} has invalid port ${String(entry.port)}`,
        );
      }
      if (entry.service.length === 0) {
        throw new EConfig(
          `plugin ${this.name} vsock service name must not be empty`,
        );
      }
    }
    if (this.scopeValue !== undefined) {
      validateScope(this.scopeValue, this.name);
    }
    this.built = true;
    return this.snapshot();
  }

  private snapshot(): BuiltPlugin<Name, Ctx, Deps> {
    const hooks = Object.freeze([...this.hooks]);
    const scope = this.scopeValue;
    const forkFn = this.forkFn;
    const contract = this;
    const requiredDeps = Object.freeze([...this.requiredDeps]);
    const deriveFns = Object.freeze([...this.deriveFns]);
    const contextFns = Object.freeze([...this.contextFns]);
    const vsockServices = Object.freeze([...this.vsockEntries]);
    const options = this.storedOptions as
      | { readonly enabled?: boolean; readonly config?: unknown }
      | undefined;
    return Object.freeze({
      name: this.name,
      enabled: options?.enabled !== false,
      config: options?.config,
      scope,
      requiredDeps,
      deriveFns,
      contextFns,
      vsockServices,
      hooksForFusion(): readonly HookDescriptor[] {
        return hooks;
      },
      registerInterceptors(bind: (hook: HookDescriptor) => void): void {
        for (const hook of hooks) {
          bind(hook);
        }
      },
      forkState(): BuiltPlugin<Name, Ctx, Deps> {
        if (forkFn !== undefined) {
          return forkFn().build();
        }
        return contract.snapshot();
      },
    });
  }

  // see: cans/02-plugin-model.md#Foreign-plugin-proxy
  private addHook(
    ring: RingEvent,
    run: (event: unknown) => unknown,
    lifecycleTag?: LifecycleTag,
  ): void {
    this.hooks.push({
      ring,
      stage: "single",
      pluginName: this.name,
      ordinal: this.nextOrdinal,
      lifecycleTag,
      run,
    });
    this.nextOrdinal += 1;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  trap(
    fn: (scope: TrapScope<Ctx>) => TrapScope<Ctx>,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".trap");
    const contract = this;
    const scope: TrapScope<Ctx> = {
      onTrap(handler) {
        contract.onTrap(handler);
        return scope;
      },
      onProcessExit(handler) {
        contract.onProcessExit(handler);
        return scope;
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  fs(
    fn: (scope: FsScope<Ctx>) => FsScope<Ctx>,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".fs");
    const contract = this;
    const scope: FsScope<Ctx> = {
      onFsLookup(handler) {
        contract.onFsLookup(handler);
        return scope;
      },
      onFsRead(handler) {
        contract.onFsRead(handler);
        return scope;
      },
      onFsWrite(handler) {
        contract.onFsWrite(handler);
        return scope;
      },
      onFsLock(handler) {
        contract.onFsLock(handler);
        return scope;
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  net(
    fn: (scope: NetScope<Ctx>) => NetScope<Ctx>,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".net");
    const contract = this;
    const scope: NetScope<Ctx> = {
      onNetOutbound(handler) {
        contract.onNetOutbound(handler);
        return scope;
      },
      onNetInbound(handler) {
        contract.onNetInbound(handler);
        return scope;
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  console(
    fn: (scope: ConsoleScope<Ctx>) => ConsoleScope<Ctx>,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".console");
    const contract = this;
    const scope: ConsoleScope<Ctx> = {
      onConsoleInput(handler) {
        contract.onConsoleInput(handler);
        return scope;
      },
      onConsoleOutput(handler) {
        contract.onConsoleOutput(handler);
        return scope;
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  exec(
    fn: (scope: ExecScope<Ctx>) => ExecScope<Ctx>,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".exec");
    const contract = this;
    const scope: ExecScope<Ctx> = {
      onExecStdout(handler) {
        contract.onExecStdout(handler);
        return scope;
      },
      onExecStderr(handler) {
        contract.onExecStderr(handler);
        return scope;
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  host(
    fn: (scope: HostScope<Ctx>) => HostScope<Ctx>,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".host");
    const contract = this;
    const scope: HostScope<Ctx> = {
      onHostLsmExec(handler) {
        contract.onHostLsmExec(handler);
        return scope;
      },
      onHostPressure(handler) {
        contract.onHostPressure(handler);
        return scope;
      },
      onHostKeyringAccess(handler) {
        contract.onHostKeyringAccess(handler);
        return scope;
      },
      onHostPreempt(handler) {
        contract.onHostPreempt(handler);
        return scope;
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  lifecycle(
    fn: (scope: LifecycleScope<Ctx>) => LifecycleScope<Ctx>,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".lifecycle");
    const contract = this;
    const scope: LifecycleScope<Ctx> = {
      onVmInit(handler) {
        contract.onVmInit(handler);
        return scope;
      },
      onVmReady(handler) {
        contract.onVmReady(handler);
        return scope;
      },
      onVmShutdown(handler) {
        contract.onVmShutdown(handler);
        return scope;
      },
      onPreCommit(handler) {
        contract.onPreCommit(handler);
        return scope;
      },
      onPostCommit(handler) {
        contract.onPostCommit(handler);
        return scope;
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  telemetry(
    fn: (scope: TelemetryScope<Ctx>) => TelemetryScope<Ctx>,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".telemetry");
    const contract = this;
    const scope: TelemetryScope<Ctx> = {
      onMemoryPressure(handler) {
        contract.onMemoryPressure(handler);
        return scope;
      },
      onHeartbeat(handler) {
        contract.onHeartbeat(handler);
        return scope;
      },
      onWatchdog(handler) {
        contract.onWatchdog(handler);
        return scope;
      },
    };
    fn(scope);
    return this;
  }

  onNetOutbound(
    handler: (frame: Uint8Array) => FilterAction,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onNetOutbound");
    this.addHook(RingEvent.NetOutbound, (event) =>
      handler(unpack(event)[0] as Uint8Array),
    );
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onNetInbound(
    handler: (frame: Uint8Array) => FilterAction,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onNetInbound");
    this.addHook(RingEvent.NetInbound, (event) =>
      handler(unpack(event)[0] as Uint8Array),
    );
    return this;
  }

  onFsLookup(
    handler: (path: string) => Verdict,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onFsLookup");
    this.addHook(RingEvent.FsLookup, (event) =>
      handler(unpack(event)[0] as string),
    );
    return this;
  }

  onFsRead(
    handler: (path: string, buffer: Uint8Array) => Uint8Array,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onFsRead");
    this.addHook(RingEvent.FsRead, (event) => {
      const args = unpack(event);
      return handler(args[0] as string, args[1] as Uint8Array);
    });
    return this;
  }

  onFsWrite(
    handler: (
      path: string,
      buffer: Uint8Array,
      pid: number,
      ctx: TrapContext & Ctx,
    ) => FilterAction,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onFsWrite");
    this.addHook(RingEvent.FsWrite, (event) => {
      const args = unpack(event);
      return handler(
        args[0] as string,
        args[1] as Uint8Array,
        args[2] as number,
        args[3] as TrapContext & Ctx,
      );
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
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onFsLock");
    this.addHook("fs-lock" as RingEvent, (event) => {
      const args = unpack(event);
      return handler(
        args[0] as string,
        args[1] as number,
        args[2] as number,
        args[3] as number,
      );
    });
    return this;
  }

  onConsoleInput(
    handler: (
      port: number,
      bytes: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onConsoleInput");
    this.addHook(RingEvent.ConsoleInput, (event) => {
      const args = unpack(event);
      return handler(args[0] as number, args[1] as Uint8Array);
    });
    return this;
  }

  onConsoleOutput(
    handler: (
      port: number,
      chunk: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onConsoleOutput");
    this.addHook(RingEvent.ConsoleOutput, (event) => {
      const args = unpack(event);
      return handler(args[0] as number, args[1] as Uint8Array);
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
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onExecStdout");
    this.addHook(RingEvent.ExecStdout, (event) => {
      const args = unpack(event);
      return handler(
        args[0] as number,
        args[1] as number,
        args[2] as Uint8Array,
      );
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
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onExecStderr");
    this.addHook(RingEvent.ExecStderr, (event) => {
      const args = unpack(event);
      return handler(
        args[0] as number,
        args[1] as number,
        args[2] as Uint8Array,
      );
    });
    return this;
  }

  onHeartbeat(
    handler: (event: HeartbeatEvent) => Promise<void> | void,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onHeartbeat");
    this.addHook(RingEvent.Heartbeat, (event) =>
      handler(unpack(event)[0] as HeartbeatEvent),
    );
    return this;
  }

  onProcessExit(
    handler: (event: ProcessExitEvent) => Promise<void> | void,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onProcessExit");
    this.addHook(RingEvent.ProcessExit, (event) =>
      handler(unpack(event)[0] as ProcessExitEvent),
    );
    return this;
  }

  onTrap(
    handler: (
      event: TrapInfo,
      c: TrapContext & Ctx,
    ) => Promise<Verdict | null> | Verdict | null,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onTrap");
    this.addHook(RingEvent.Trap, (event) => {
      const args = unpack(event);
      return handler(args[0] as TrapInfo, args[1] as TrapContext & Ctx);
    });
    return this;
  }

  onWatchdog(
    handler: (
      event: WatchdogEvent,
    ) => Promise<WatchdogVerdict> | WatchdogVerdict,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onWatchdog");
    this.addHook(RingEvent.Watchdog, (event) =>
      handler(unpack(event)[0] as WatchdogEvent),
    );
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onHostLsmExec(
    handler: (path: string, pid: number) => Verdict,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onHostLsmExec");
    this.addHook("host-lsm-exec" as RingEvent, (event) => {
      const args = unpack(event);
      return handler(args[0] as string, args[1] as number);
    });
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onHostPressure(
    handler: (level: number) => Verdict,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onHostPressure");
    this.addHook("host-pressure" as RingEvent, (event) =>
      handler(unpack(event)[0] as number),
    );
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onHostKeyringAccess(
    handler: (key: string) => Verdict,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onHostKeyringAccess");
    this.addHook("host-keyring-access" as RingEvent, (event) =>
      handler(unpack(event)[0] as string),
    );
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onVmInit(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onVmInit");
    this.addHook(
      RingEvent.Lifecycle,
      (event) => handler(unpack(event)[0] as LifecycleContext),
      "vm-init",
    );
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onVmReady(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onVmReady");
    this.addHook(
      RingEvent.Lifecycle,
      (event) => handler(unpack(event)[0] as LifecycleContext),
      "vm-ready",
    );
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onVmShutdown(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onVmShutdown");
    this.addHook(
      RingEvent.Lifecycle,
      (event) => handler(unpack(event)[0] as LifecycleContext),
      "vm-shutdown",
    );
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onPreCommit(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onPreCommit");
    this.addHook(
      RingEvent.Lifecycle,
      (event) => handler(unpack(event)[0] as LifecycleContext),
      "pre-commit",
    );
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onPostCommit(
    handler: (
      ctx: LifecycleContext,
      committed: boolean,
    ) => Promise<void> | void,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onPostCommit");
    this.addHook(
      RingEvent.Lifecycle,
      (event) => {
        const args = unpack(event);
        return handler(args[0] as LifecycleContext, args[1] as boolean);
      },
      "post-commit",
    );
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onHostPreempt(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onHostPreempt");
    this.addHook("host-preempt" as RingEvent, (event) =>
      handler(unpack(event)[0] as LifecycleContext),
    );
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onMemoryPressure(
    handler: (
      availableMb: number,
      totalMb: number,
    ) => Promise<void> | void,
  ): PluginContract<Name, Ctx, Deps> {
    this.checkOpen(".onMemoryPressure");
    this.addHook("memory-pressure" as RingEvent, (event) => {
      const args = unpack(event);
      return handler(args[0] as number, args[1] as number);
    });
    return this;
  }
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
export function createPlugin<
  Name extends string,
  Ctx = Record<string, unknown>,
  Deps extends string = never,
>(name: Name, options?: Ctx | CreatePluginOptions): PluginContract<Name, Ctx, Deps> {
  return new PluginContract<Name, Ctx, Deps>(name, options);
}
