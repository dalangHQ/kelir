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
import type { LockBrokerConfig } from "./builder.config.js";
import type { Session } from "./session.lifecycle.js";
import type { KelirBuilder } from "./builder.lifecycle.js";
import { unknownSchema, validateSchema } from "./schema.config.js";
import type {
  FilterAction,
  DnsVerdict,
  Verdict,
} from "./verdict.protocol.js";
import type {
  DirEntry,
  HeartbeatEvent,
  ProcessExitEvent,
  TrapDenyEvent,
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
  onSignal(
    handler: (
      sender: number,
      target: number,
      sig: number,
    ) => Promise<Verdict> | Verdict,
  ): TrapScope<Ctx, Owner>;
  onFork(
    handler: (
      parent: number,
      child: number,
    ) => Promise<Verdict> | Verdict,
  ): TrapScope<Ctx, Owner>;
  end(): Owner;
}

// see: cans/02-plugin-model.md#Nested-scopes
export interface FsScopeConfig {
  readonly lockBroker?: LockBrokerConfig;
}

// see: cans/02-plugin-model.md#Nested-scopes
export interface ScopeRules {
  readonly read: { readonly allow: readonly string[]; readonly deny: readonly string[] };
  readonly write: { readonly mask: readonly string[] };
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
  onFsUnlink(
    handler: (path: string, ctx: TrapContext & Ctx) => Verdict,
  ): FsScope<Ctx, Owner>;
  onFsRename(
    handler: (src: string, dst: string, ctx: TrapContext & Ctx) => Verdict,
  ): FsScope<Ctx, Owner>;
  onFsSetattr(
    handler: (path: string, mode: number, size: number) => Verdict,
  ): FsScope<Ctx, Owner>;
  onFsSymlink(
    handler: (target: string, link: string) => Verdict,
  ): FsScope<Ctx, Owner>;
  onFsMkdir(
    handler: (path: string, mode: number) => Verdict,
  ): FsScope<Ctx, Owner>;
  onFsReaddir(
    handler: (path: string, entries: DirEntry[]) => FilterAction,
  ): FsScope<Ctx, Owner>;
  onFsReadlink(
    handler: (path: string) => Verdict,
  ): FsScope<Ctx, Owner>;
  // see: cans/02-plugin-model.md#Nested-scopes
  config(cfg: FsScopeConfig): FsScope<Ctx, Owner>;
  // see: cans/02-plugin-model.md#Nested-scopes
  requires<Req extends string>(name?: Req): FsScope<Ctx, Owner>;
  // see: cans/02-plugin-model.md#Nested-scopes
  forkWith(fn: () => KelirBuilder<any, any, any, any, any, any>): FsScope<Ctx, Owner>;
  // see: cans/02-plugin-model.md#Nested-scopes
  timeout(seconds: number): FsScope<Ctx, Owner>;
  // see: cans/02-plugin-model.md#Nested-scopes
  priority(i: number): FsScope<Ctx, Owner>;
  // see: cans/02-plugin-model.md#Nested-scopes
  rules(rules: ScopeRules): FsScope<Ctx, Owner>;
  end(): Owner;
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export interface NetScope<Ctx, Owner = unknown> {
  onNetOutbound(handler: (frame: Uint8Array) => FilterAction): NetScope<Ctx, Owner>;
  onNetInbound(handler: (frame: Uint8Array) => FilterAction): NetScope<Ctx, Owner>;
  onSocketConnect(
    handler: (
      pid: number,
      proto: number,
      ip: readonly [number, number, number, number],
      port: number,
    ) => Verdict,
  ): NetScope<Ctx, Owner>;
  onDnsQuery(
    handler: (pid: number, domain: string, qtype: number) => DnsVerdict,
  ): NetScope<Ctx, Owner>;
  onSocketBind(
    handler: (
      pid: number,
      proto: number,
      ip: readonly [number, number, number, number],
      port: number,
    ) => Verdict,
  ): NetScope<Ctx, Owner>;
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
  onExecStdin(
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
  onHostContainmentBreach(
    handler: (
      pid: number,
      path: string,
      kind: string,
    ) => Promise<void> | void,
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

// see: cans/artifacts/code/typescript-scope-interface.ts#ActionsScope
export interface ActionsScope<Ctx = never, Owner = unknown> {
  register<P>(
    name: string,
    schema: Schema<P>,
    handler: (payload: P) => Promise<void> | void,
  ): ActionsScope<Ctx, Owner>;
}

// see: cans/artifacts/code/typescript-scope-interface.ts#StreamsScope
export interface StreamsScope<Ctx = never, Owner = unknown> {
  register(
    kind: number,
    schema: Schema<unknown>,
    handler: (request: Uint8Array) => Promise<Uint8Array> | Uint8Array,
  ): StreamsScope<Ctx, Owner>;
}

// see: cans/artifacts/code/typescript-scope-interface.ts#VsockScope
export interface VsockScope<Ctx = never, Owner = unknown> {
  service(
    port: number,
    service: string,
    handler: (request: unknown) => Promise<unknown> | unknown,
  ): VsockScope<Ctx, Owner>;
}

// see: cans/artifacts/code/typescript-scope-interface.ts#ParamsScope
export interface ParamsScope<Ctx = never, Owner = unknown> {
  model<S extends Schema<any>>(schema: S): ParamsScope<Ctx, Owner>;
}

// see: cans/artifacts/code/typescript-scope-interface.ts#DenyScope
export interface DenyScope<Ctx = never, Owner = unknown> {
  onTrapDeny(
    handler: (
      event: TrapDenyEvent,
      verdict: Verdict,
      session: Session & Ctx,
    ) => Promise<void> | void,
  ): DenyScope<Ctx, Owner>;
}

function unpack(event: unknown): readonly unknown[] {
  return Array.isArray(event) ? (event as readonly unknown[]) : [event];
}

// see: cans/02-plugin-model.md#Segregated-ring-traits
const KNOWN_RINGS: ReadonlySet<string> = new Set<string>([
  ...Object.values(RingEvent),
  "fs-lock",
  "fs-unlink",
  "fs-rename",
  "fs-setattr",
  "fs-symlink",
  "fs-mkdir",
  "fs-readdir",
  "fs-readlink",
  "signal",
  "fork",
  "socket-connect",
  "dns-query",
  "socket-bind",
  "exec-stdin",
  "host-containment-breach",
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
  fsScopeConfig?: FsScopeConfig;
  fsScopeForkFn?: () => KelirBuilder<any, any, any, any, any, any>;
  fsScopeTimeoutSeconds?: number;
  fsScopePriority?: number;
  fsScopeRules?: ScopeRules;

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
    copy.fsScopeConfig = this.fsScopeConfig;
    copy.fsScopeForkFn = this.fsScopeForkFn;
    copy.fsScopeTimeoutSeconds = this.fsScopeTimeoutSeconds;
    copy.fsScopePriority = this.fsScopePriority;
    copy.fsScopeRules = this.fsScopeRules;
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
      onSignal(handler) {
        registry.onSignal(handler);
        return scope;
      },
      onFork(handler) {
        registry.onFork(handler);
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
    pluginMode?: boolean,
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
      onFsUnlink(handler) {
        registry.onFsUnlink(handler);
        return scope;
      },
      onFsRename(handler) {
        registry.onFsRename(handler);
        return scope;
      },
      onFsSetattr(handler) {
        registry.onFsSetattr(handler);
        return scope;
      },
      onFsSymlink(handler) {
        registry.onFsSymlink(handler);
        return scope;
      },
      onFsMkdir(handler) {
        registry.onFsMkdir(handler);
        return scope;
      },
      onFsReaddir(handler) {
        registry.onFsReaddir(handler);
        return scope;
      },
      onFsReadlink(handler) {
        registry.onFsReadlink(handler);
        return scope;
      },
      config(cfg) {
        if (pluginMode === true) {
          throw new EConfig(`${registry.owner} called host-only verb fs.config`);
        }
        registry.fsScopeConfig = cfg;
        return scope;
      },
      requires(name) {
        if (name !== undefined && !registry.requiredDeps.includes(name)) {
          registry.requiredDeps.push(name);
        }
        return scope;
      },
      forkWith(fn) {
        registry.fsScopeForkFn = fn;
        return scope;
      },
      timeout(seconds) {
        registry.fsScopeTimeoutSeconds = seconds;
        return scope;
      },
      priority(i) {
        registry.fsScopePriority = i;
        return scope;
      },
      rules(rules) {
        registry.fsScopeRules = rules;
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
      onSocketConnect(handler) {
        registry.onSocketConnect(handler);
        return scope;
      },
      onDnsQuery(handler) {
        registry.onDnsQuery(handler);
        return scope;
      },
      onSocketBind(handler) {
        registry.onSocketBind(handler);
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
      onExecStdin(handler) {
        registry.onExecStdin(handler);
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
      onHostContainmentBreach(handler) {
        registry.onHostContainmentBreach(handler);
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

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onSocketConnect(
    handler: (
      pid: number,
      proto: number,
      ip: readonly [number, number, number, number],
      port: number,
    ) => Verdict,
  ): this {
    // TODO (L3) [bindings_ts]: live dispatch of the socket-connect ring; the descriptor records the handler until the native bridge carries it.; slop: socket-connect handlers never fire, the ring runs handler-less.
    this.addHook({
      ring: "socket-connect" as RingEvent,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(
          args[0] as number,
          args[1] as number,
          args[2] as readonly [number, number, number, number],
          args[3] as number,
        );
      },
    });
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onDnsQuery(
    handler: (pid: number, domain: string, qtype: number) => DnsVerdict,
  ): this {
    // TODO (L3) [bindings_ts]: live dispatch of the dns-query ring; the descriptor records the handler until the native bridge carries it.; slop: dns-query handlers never fire, the ring runs handler-less.
    this.addHook({
      ring: "dns-query" as RingEvent,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(args[0] as number, args[1] as string, args[2] as number);
      },
    });
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onSocketBind(
    handler: (
      pid: number,
      proto: number,
      ip: readonly [number, number, number, number],
      port: number,
    ) => Verdict,
  ): this {
    // TODO (L3) [bindings_ts]: live dispatch of the socket-bind ring; the descriptor records the handler until the native bridge carries it.; slop: socket-bind handlers never fire, the ring runs handler-less.
    this.addHook({
      ring: "socket-bind" as RingEvent,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(
          args[0] as number,
          args[1] as number,
          args[2] as readonly [number, number, number, number],
          args[3] as number,
        );
      },
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
          args[1] as Buffer,
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

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onFsUnlink(
    handler: (path: string, ctx: TrapContext & Ctx) => Verdict,
  ): this {
    this.addHook({
      ring: "fs-unlink" as RingEvent,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(args[0] as string, args[1] as TrapContext & Ctx);
      },
    });
    return this;
  }

  onFsRename(
    handler: (src: string, dst: string, ctx: TrapContext & Ctx) => Verdict,
  ): this {
    this.addHook({
      ring: "fs-rename" as RingEvent,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(
          args[0] as string,
          args[1] as string,
          args[2] as TrapContext & Ctx,
        );
      },
    });
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onFsSetattr(
    handler: (path: string, mode: number, size: number) => Verdict,
  ): this {
    // TODO (L3) [bindings_ts]: live dispatch of the fs-setattr ring; the descriptor records the handler until the native bridge carries it.; slop: fs-setattr handlers never fire, the ring runs handler-less.
    this.addHook({
      ring: "fs-setattr" as RingEvent,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(args[0] as string, args[1] as number, args[2] as number);
      },
    });
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onFsSymlink(
    handler: (target: string, link: string) => Verdict,
  ): this {
    // TODO (L3) [bindings_ts]: live dispatch of the fs-symlink ring; the descriptor records the handler until the native bridge carries it.; slop: fs-symlink handlers never fire, the ring runs handler-less.
    this.addHook({
      ring: "fs-symlink" as RingEvent,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(args[0] as string, args[1] as string);
      },
    });
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onFsMkdir(
    handler: (path: string, mode: number) => Verdict,
  ): this {
    // TODO (L3) [bindings_ts]: live dispatch of the fs-mkdir ring; the descriptor records the handler until the native bridge carries it.; slop: fs-mkdir handlers never fire, the ring runs handler-less.
    this.addHook({
      ring: "fs-mkdir" as RingEvent,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(args[0] as string, args[1] as number);
      },
    });
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onFsReaddir(
    handler: (path: string, entries: DirEntry[]) => FilterAction,
  ): this {
    // TODO (L3) [bindings_ts]: live dispatch of the fs-readdir ring; the descriptor records the handler until the native bridge carries it.; slop: fs-readdir handlers never fire, the ring runs handler-less.
    this.addHook({
      ring: "fs-readdir" as RingEvent,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(args[0] as string, args[1] as DirEntry[]);
      },
    });
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onFsReadlink(
    handler: (path: string) => Verdict,
  ): this {
    // TODO (L3) [bindings_ts]: live dispatch of the fs-readlink ring; the descriptor records the handler until the native bridge carries it.; slop: fs-readlink handlers never fire, the ring runs handler-less.
    this.addHook({
      ring: "fs-readlink" as RingEvent,
      stage: "single",
      run: (event) => handler(unpack(event)[0] as string),
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

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onExecStdin(
    handler: (
      pid: number,
      execId: number,
      chunk: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): this {
    // TODO (L3) [bindings_ts]: live dispatch of the exec-stdin ring; the descriptor records the handler until the native bridge carries it.; slop: exec-stdin handlers never fire, the ring runs handler-less.
    this.addHook({
      ring: "exec-stdin" as RingEvent,
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

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onSignal(
    handler: (
      sender: number,
      target: number,
      sig: number,
    ) => Promise<Verdict> | Verdict,
  ): this {
    // TODO (L3) [bindings_ts]: live dispatch of the signal ring; the descriptor records the handler until the native bridge carries it.; slop: signal handlers never fire, the ring runs handler-less.
    this.addHook({
      ring: "signal" as RingEvent,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(args[0] as number, args[1] as number, args[2] as number);
      },
    });
    return this;
  }

  // see: cans/02-plugin-model.md#Segregated-ring-traits
  onFork(
    handler: (
      parent: number,
      child: number,
    ) => Promise<Verdict> | Verdict,
  ): this {
    // TODO (L3) [bindings_ts]: live dispatch of the fork ring; the descriptor records the handler until the native bridge carries it.; slop: fork handlers never fire, the ring runs handler-less.
    this.addHook({
      ring: "fork" as RingEvent,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(args[0] as number, args[1] as number);
      },
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
  onHostContainmentBreach(
    handler: (
      pid: number,
      path: string,
      kind: string,
    ) => Promise<void> | void,
  ): this {
    // TODO (L3) [bindings_ts]: live dispatch of the host-containment-breach ring; the descriptor records the handler until the native bridge carries it.; slop: host-containment-breach handlers never fire, the ring runs handler-less.
    this.addHook({
      ring: "host-containment-breach" as RingEvent,
      stage: "single",
      run: (event) => {
        const args = unpack(event);
        return handler(args[0] as number, args[1] as string, args[2] as string);
      },
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

  // see: cans/artifacts/code/typescript-scope-interface.ts#ActionsScope
  registerCustomAction<P>(
    name: string,
    schema: Schema<P>,
    handler: (payload: P) => Promise<void> | void,
  ): this {
    this.addHook({
      ring: RingEvent.CustomAction,
      stage: "single",
      customActionName: name,
      run: (payload: unknown): unknown =>
        handler(validateSchema(schema, payload)),
    });
    return this;
  }

  // see: cans/11-protocol.md#Control-channel
  registerStreamKind<Req, Resp = unknown>(
    kind: number,
    schema: Schema<Req>,
    handler: (request: Req) => Promise<Resp> | Resp,
  ): this {
    this.addHook({
      ring: RingEvent.VsockService,
      stage: "single",
      vsockKind: kind,
      run: (payload: unknown): unknown =>
        handler(validateSchema(schema, payload)),
    });
    return this;
  }

  // see: cans/artifacts/code/typescript-scope-interface.ts#StreamsScope
  registerStreamKindBytes(
    kind: number,
    schema: Schema<unknown>,
    handler: (request: Uint8Array) => Promise<Uint8Array> | Uint8Array,
  ): this {
    // TODO (L3) [bt_03_typescript_fused_trampoline_single_crossing_chained_hooks]: validate the request bytes against schema once the stream dispatch decodes them; slop: unvalidated request bytes reach the stream handler.
    void schema;
    this.addHook({
      ring: RingEvent.VsockService,
      stage: "single",
      vsockKind: kind,
      run: (payload: unknown): unknown => handler(payload as Uint8Array),
    });
    return this;
  }

  // see: cans/11-protocol.md#Control-channel
  vsockService<Req, Resp = unknown>(
    port: number,
    service: string,
    schema: Schema<Req>,
    handler: (request: Req) => Promise<Resp> | Resp,
  ): this {
    if (!Number.isInteger(port) || port < 0 || port > 0xffffffff) {
      throw new EConfig(`vsock service port ${String(port)} outside u32`);
    }
    this.vsockEntries.push({
      port,
      service,
      schema,
      handler: (request: unknown): Promise<unknown> | unknown =>
        handler(validateSchema(schema, request)),
    });
    return this;
  }

  // see: cans/artifacts/code/typescript-scope-interface.ts#DenyScope
  onTrapDeny(
    handler: (
      event: TrapDenyEvent,
      verdict: Verdict,
      session: Session & Ctx,
    ) => Promise<void> | void,
  ): this {
    this.addHook({
      ring: RingEvent.Lifecycle,
      stage: "single",
      lifecycleTag: "trap-deny",
      run: (payload: unknown): unknown => {
        const args = unpack(payload);
        return Promise.resolve(
          handler(
            args[0] as TrapDenyEvent,
            args[1] as Verdict,
            args[2] as Session & Ctx,
          ),
        ).catch((error: unknown): never => {
          throw EConfig.fromNative(error);
        });
      },
    });
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  actions<Owner>(
    fn: (scope: ActionsScope<Ctx, Owner>) => ActionsScope<Ctx, Owner> | void,
  ): this {
    const registry = this;
    const scope: ActionsScope<Ctx, Owner> = {
      register<P>(
        name: string,
        schema: Schema<P>,
        handler: (payload: P) => Promise<void> | void,
      ): ActionsScope<Ctx, Owner> {
        registry.registerCustomAction(name, schema, handler);
        return scope;
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  streams<Owner>(
    fn: (scope: StreamsScope<Ctx, Owner>) => StreamsScope<Ctx, Owner> | void,
  ): this {
    const registry = this;
    const scope: StreamsScope<Ctx, Owner> = {
      register(
        kind: number,
        schema: Schema<unknown>,
        handler: (request: Uint8Array) => Promise<Uint8Array> | Uint8Array,
      ): StreamsScope<Ctx, Owner> {
        registry.registerStreamKindBytes(kind, schema, handler);
        return scope;
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  vsock<Owner>(
    fn: (scope: VsockScope<Ctx, Owner>) => VsockScope<Ctx, Owner> | void,
  ): this {
    const registry = this;
    const scope: VsockScope<Ctx, Owner> = {
      service(
        port: number,
        service: string,
        handler: (request: unknown) => Promise<unknown> | unknown,
      ): VsockScope<Ctx, Owner> {
        registry.vsockService(port, service, unknownSchema, handler);
        return scope;
      },
    };
    fn(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  deny<Owner>(
    fn: (scope: DenyScope<Ctx, Owner>) => DenyScope<Ctx, Owner> | void,
  ): this {
    const registry = this;
    const scope: DenyScope<Ctx, Owner> = {
      onTrapDeny(
        handler: (
          event: TrapDenyEvent,
          verdict: Verdict,
          session: Session & Ctx,
        ) => Promise<void> | void,
      ): DenyScope<Ctx, Owner> {
        registry.onTrapDeny(handler);
        return scope;
      },
    };
    fn(scope);
    return this;
  }
}
