// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/02-plugin-model.md#Foreign-plugin-proxy
// see: cans/artifacts/code/typescript-accumulation.ts#runtime

import type { Schema, Infer } from "./schema.config.js";
import { validateSchema } from "./schema.config.js";
import type { TrapContext, LifecycleContext, HandlerContext } from "./context.lifecycle.types.js";
import { HookRegistry } from "./hook.registry.js";
import type {
  KelirPlugin,
  MountScope,
  PluginContext,
  VsockServiceEntry,
  TrapScope,
  FsScope,
  NetScope,
  ConsoleScope,
  ExecScope,
  HostScope,
  LifecycleScope,
  TelemetryScope,
} from "./hook.registry.js";
import type {
  PostExecAction,
  LifecycleEvent,
  Verdict,
  FilterAction,
} from "./verdict.protocol.js";
import type {
  TrapDenyEvent,
  TrapInfo,
  WatchdogEvent,
  WatchdogVerdict,
  HeartbeatEvent,
  ProcessExitEvent,
} from "./types.core.js";
import { Session } from "./session.lifecycle.js";
import { EConfig } from "./errors.sdk.js";
import { TrapFilter } from "./trap.protocol.js";
import {
  FusedTrampoline,
  RingEvent,
  lifecycleTagFor,
  type FusedTrampolineDescriptor,
  type HookDescriptor,
  type NativeAddonBinding,
} from "./fusion.plugin.js";

export interface KelirConfig {
  readonly vcpus?: number;
  readonly memoryMb?: number;
  readonly pageSizeKb?: number;
  readonly deterministicClock?: boolean;
  readonly pinnedClocksource?: string;
  readonly preemptionGraceMs?: number;
  readonly driver?: string;
  readonly kernel?: string;
  readonly rootfsErofs?: string;
  readonly tscLocked?: boolean;
  readonly enableWatchdog?: boolean;
  readonly detached?: boolean;
}

// see: cans/04-terminal-plane.md#Terminal-pipeline
export interface TerminalStream {
  write(data: string): void;
}

// see: cans/04-terminal-plane.md#Terminal-pipeline
export interface TerminalConfig {
  readonly cols?: number;
  readonly rows?: number;
  readonly dropOsc52?: boolean;
  readonly stripTitle?: boolean;
  readonly tokenShield?: {
    readonly maxBytesTotal: number;
    readonly maxLinesTotal: number;
    readonly clampStrategy: string;
    readonly onClamp?: (stats: {
      droppedBytes: number;
      droppedLines: number;
    }) => string;
  };
  readonly prompts?: readonly {
    readonly match: RegExp;
    readonly timeoutMs: number;
    readonly verifyForeground: boolean;
    readonly action: (
      prompt: unknown,
      stream: TerminalStream,
    ) => Promise<void> | void;
  }[];
  readonly streamEngine?: string;
}

// see: cans/05-storage-plane.md#Workspace-commit
export interface StorageConfig<Ctx = unknown> {
  readonly idmap?: { readonly guestUid: number; readonly hostUid: number };
  readonly sharedCaches?: readonly string[];
  readonly lockBroker?: boolean;
  readonly syntheticFiles?: readonly {
    readonly path: string;
    readonly content: string | (() => string);
    readonly mode?: number;
  }[];
  readonly maskRead?: readonly {
    readonly glob: string;
    readonly pattern: RegExp;
    readonly replace: string;
  }[];
  readonly snoops?: readonly {
    readonly path: string;
    readonly sync: boolean;
    readonly onWrite: (
      path: string,
      raw: Uint8Array,
      c: TrapContext & Ctx,
    ) => Promise<void> | void;
  }[];
  readonly encapsulation?: string;
  readonly scanEngine?: string;
  readonly carryoverBytes?: number;
}

// see: cans/07-wire-plane.md#DNS-policy-filter
export interface WireConfig {
  readonly sinkhole?: { readonly dnsExcept: readonly string[] };
  readonly sshAgentBridge?: boolean;
  readonly vaults?: readonly {
    readonly name: string;
    readonly kind: "bearer_token" | "sigv4" | "mtls_bundle";
    readonly audience: string;
    readonly placeholder: string;
  }[];
  readonly l4Proxies?: readonly {
    readonly listenPort: number;
    readonly destination: string;
    readonly injectMtls?: {
      readonly certPath: string;
      readonly keyPath: string;
    };
  }[];
  readonly tapFilter?: string;
}

export type TrapHandlerResult = Verdict | PostExecAction;

export type TrapHandler<Ctx, Params> = (
  c: HandlerContext<Ctx, Params>,
) => Promise<TrapHandlerResult> | TrapHandlerResult;

export interface TrapHandlers<Ctx, Params> {
  readonly pre?: TrapHandler<Ctx, Params>;
  readonly post?: (c: HandlerContext<Ctx, Params>) => Promise<PostExecAction> | PostExecAction;
}

interface PluginRegistration {
  readonly name: string;
  readonly enabled: boolean;
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
export interface CreatePluginOptions<Opts = unknown> {
  readonly opts?: Opts;
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
export type ValidateDeps<Ctx, P> = P extends KelirBuilder<any, any, infer Deps, any>
  ? [Deps] extends [never]
    ? unknown
    : string extends Deps
      ? unknown
      : [Deps] extends [keyof Ctx]
        ? unknown
        : { readonly __missing_dependency: Deps }
  : unknown;

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
export type PluginContextOf<P> =
  P extends KelirBuilder<infer Name, infer C, any, any>
    ? (C extends Record<string, unknown>
        ? (string extends keyof C
            ? { readonly [K in Name]: C }
            : (Name extends keyof C ? C : { readonly [K in Name]: C }))
        : { readonly [K in Name]: C })
    : P extends PluginContext<infer C>
      ? C
      : unknown;

// see: cans/02-plugin-model.md#Foreign-plugin-proxy
// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
export class KelirBuilder<Name extends string = string, Ctx = unknown, Deps extends string = never, Params = unknown> {
  private readonly mode: "host" | "plugin";
  readonly name: string;
  private readonly storedOptions?: Ctx | CreatePluginOptions<unknown>;
  private readonly kelirConfig: KelirConfig;
  private registry: HookRegistry<Ctx>;
  private mounted: HookRegistry<Ctx>[] = [];
  private pluginRegistrations: PluginRegistration[] = [];
  private mountedFlag = false;
  private forkFn?: () => KelirBuilder<Name, Ctx, Deps, Params>;
  private detachedFlag: boolean;
  private terminalCfg: TerminalConfig | undefined;
  private storageCfg: StorageConfig<any> | undefined;
  private wireCfg: WireConfig | undefined;
  private paramsSchema: Schema<unknown> | undefined;

  constructor(config?: KelirConfig);
  constructor(name: string, options?: Ctx | CreatePluginOptions<unknown>);
  constructor(configOrName: KelirConfig | string = {}, options?: Ctx | CreatePluginOptions<unknown>) {
    if (typeof configOrName === "string") {
      this.mode = "plugin";
      this.name = configOrName;
      this.storedOptions = options;
      this.kelirConfig = {};
      this.registry = new HookRegistry<Ctx>(configOrName);
    } else {
      this.mode = "host";
      this.name = "";
      this.storedOptions = undefined;
      this.kelirConfig = configOrName;
      this.registry = new HookRegistry<Ctx>();
    }
    this.detachedFlag = this.kelirConfig.detached ?? false;
  }

  get requiredDeps(): readonly string[] {
    return this.registry.requiredDeps;
  }

  get deriveFns(): readonly ((c: unknown) => unknown)[] {
    return this.registry.deriveFns;
  }

  get contextFns(): readonly ((c: unknown) => unknown)[] {
    return this.registry.contextFns;
  }

  // TODO (L3): the engine dispatch reads these entries once register_interceptors wiring lands per cans/02-plugin-model.md#Static-dispatch.
  get vsockServices(): readonly VsockServiceEntry[] {
    return [
      ...this.registry.vsockEntries,
      ...this.mounted.flatMap((registry) => registry.vsockEntries),
    ];
  }

  getScope(): MountScope | undefined {
    return this.registry.getScope();
  }

  private checkMounted(verb: string): void {
    if (this.mountedFlag) {
      throw new EConfig(`plugin ${this.name} is already mounted: ${verb} after .use()`);
    }
  }

  private hostOnlyRejected(verb: string): boolean {
    if (this.mode === "plugin") {
      this.registry.recordHostOnly(verb);
      return true;
    }
    return false;
  }

  // see: cans/04-terminal-plane.md#Terminal-pipeline
  terminal(config: TerminalConfig): this {
    if (this.hostOnlyRejected(".terminal")) return this;
    this.terminalCfg = config;
    return this;
  }

  // see: cans/05-storage-plane.md#Workspace-commit
  storage<C = Ctx>(config: StorageConfig<C>): KelirBuilder<Name, Ctx & C, Deps, Params> {
    if (this.hostOnlyRejected(".storage")) return this as unknown as KelirBuilder<Name, Ctx & C, Deps, Params>;
    this.storageCfg = config as StorageConfig<any>;
    return this as unknown as KelirBuilder<Name, Ctx & C, Deps, Params>;
  }

  // see: cans/07-wire-plane.md#DNS-policy-filter
  wire(config: WireConfig): this {
    if (this.hostOnlyRejected(".wire")) return this;
    this.wireCfg = config;
    return this;
  }

  // see: cans/03-sdk-architecture.md#TypeScript-SDK
  params<S extends Schema<any>>(schema: S): KelirBuilder<Name, Ctx, Deps, Infer<S>> {
    if (this.hostOnlyRejected(".params")) return this as unknown as KelirBuilder<Name, Ctx, Deps, Infer<S>>;
    this.paramsSchema = schema;
    return this as unknown as KelirBuilder<Name, Ctx, Deps, Infer<S>>;
  }

  // see: cans/08-lifecycle.md#Detached-lifecycle
  detached(detached: boolean): this {
    if (this.hostOnlyRejected(".detached")) return this;
    this.detachedFlag = detached;
    return this;
  }

  isDetached(): boolean {
    return this.detachedFlag;
  }

  // see: cans/artifacts/code/typescript-accumulation.ts#runtime
  use<P extends KelirBuilder<any, any, any, any> | KelirPlugin>(
    plugin: P & ValidateDeps<Ctx, P>,
  ): KelirBuilder<Name, Ctx & PluginContextOf<P>, Deps, Params> {
    this.checkMounted(".use");
    if (plugin instanceof KelirBuilder) {
      if (plugin.mode === "host") {
        throw new EConfig("cannot mount a host-mode builder as a plugin");
      }
      if (
        plugin.mountedFlag ||
        this.pluginRegistrations.some(
          (registration) => registration.name === plugin.name,
        )
      ) {
        throw new EConfig(`plugin already mounted: ${plugin.name}`);
      }
      const hostOnly = plugin.registry.hostOnlyCalls[0];
      if (hostOnly !== undefined) {
        throw new EConfig(
          `plugin ${plugin.name} called host-only verb ${hostOnly}`,
        );
      }
      const options = plugin.storedOptions as
        | { readonly enabled?: boolean; readonly config?: unknown }
        | undefined;
      const enabled = options?.enabled !== false;
      if (enabled) {
        for (const dep of plugin.registry.requiredDeps) {
          if (
            !this.pluginRegistrations.some(
              (registration) => registration.name === dep,
            )
          ) {
            throw new EConfig(
              `missing prerequisite: ${dep} required by ${plugin.name}`,
            );
          }
        }
      }
      for (const hook of plugin.registry.hooks) {
        this.registry.adoptHook(hook);
      }
      this.mounted.push(plugin.registry);
      for (const registration of plugin.pluginRegistrations) {
        if (
          !this.pluginRegistrations.some(
            (existing) => existing.name === registration.name,
          )
        ) {
          this.pluginRegistrations.push(registration);
        }
      }
      this.pluginRegistrations.push({ name: plugin.name, enabled });
      plugin.mountedFlag = true;
      return this as unknown as KelirBuilder<Name, Ctx & PluginContextOf<P>, Deps, Params>;
    }
    const asPlugin = plugin as unknown as KelirPlugin;
    const enabled = asPlugin.enabled !== false;
    const trait = plugin as unknown as {
      readonly requiredDeps?: readonly string[];
      readonly registerInterceptors?: (bind: (hook: HookDescriptor) => void) => void;
      readonly vsockServices?: readonly VsockServiceEntry[];
    };
    if (enabled) {
      for (const dep of trait.requiredDeps ?? []) {
        if (
          !this.pluginRegistrations.some(
            (registration) => registration.name === dep,
          )
        ) {
          throw new EConfig(
            `missing prerequisite: ${dep} required by ${asPlugin.name}`,
          );
        }
      }
    }
    this.pluginRegistrations.push({ name: asPlugin.name, enabled });
    if (typeof trait.registerInterceptors === "function" && enabled) {
      trait.registerInterceptors((hook) => {
        this.registry.adoptHook(hook);
      });
    }
    if (enabled) {
      for (const service of trait.vsockServices ?? []) {
        this.registry.vsockEntries.push(service);
      }
    }
    return this as unknown as KelirBuilder<Name, Ctx & PluginContextOf<P>, Deps, Params>;
  }

  // see: cans/02-plugin-model.md#Unified-builder
  plugin<P extends KelirBuilder<any, any, any, any> | KelirPlugin>(
    plugin: P & ValidateDeps<Ctx, P>,
  ): KelirBuilder<Name, Ctx & PluginContextOf<P>, Deps, Params> {
    return this.use(plugin);
  }

  // see: cans/03-sdk-architecture.md#TypeScript-SDK
  private checkedTrapContext(event: unknown): HandlerContext<Ctx, Params> {
    if (this.paramsSchema !== undefined) {
      if (
        typeof event !== "object" ||
        event === null ||
        !("params" in event)
      ) {
        throw new EConfig(
          "trap event carries no params for the pinned params schema",
        );
      }
      validateSchema(
        this.paramsSchema,
        (event as { readonly params: unknown }).params,
      );
    }
    return event as HandlerContext<Ctx, Params>;
  }

  // see: cans/02-plugin-model.md#Foreign-plugin-proxy
  trap(
    filter: string | TrapFilter,
    handler: TrapHandler<Ctx, Params> | TrapHandlers<Ctx, Params>,
  ): this;
  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  trap(fn: (scope: TrapScope<Ctx>) => TrapScope<Ctx>): this;
  trap(
    filterOrFn: string | TrapFilter | ((scope: TrapScope<Ctx>) => TrapScope<Ctx>),
    handler?: TrapHandler<Ctx, Params> | TrapHandlers<Ctx, Params>,
  ): this {
    this.checkMounted(".trap");
    if (typeof filterOrFn === "function") {
      this.registry.trap(filterOrFn);
      return this;
    }
    const filterSnapshot =
      typeof filterOrFn === "string" ? TrapFilter.pattern(filterOrFn).snapshot() : filterOrFn.snapshot();
    if (handler === undefined) {
      return this;
    }
    if (typeof handler === "function") {
      this.registry.addHook({
        ring: RingEvent.Trap,
        stage: "single",
        filter: filterSnapshot,
        run: (event: unknown): unknown =>
          (handler as TrapHandler<Ctx, Params>)(this.checkedTrapContext(event)),
      });
      return this;
    }
    if (handler.pre !== undefined) {
      const pre = handler.pre;
      this.registry.addHook({
        ring: RingEvent.Trap,
        stage: "pre",
        filter: filterSnapshot,
        run: (event: unknown): unknown => pre(this.checkedTrapContext(event)),
      });
    }
    if (handler.post !== undefined) {
      const post = handler.post;
      this.registry.addHook({
        ring: RingEvent.Trap,
        stage: "post",
        filter: filterSnapshot,
        run: (event: unknown): unknown => post(this.checkedTrapContext(event)),
      });
    }
    return this;
  }

  on(
    event: LifecycleEvent.VmInit,
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): this;
  on(
    event: LifecycleEvent.VmReady | LifecycleEvent.VmShutdown,
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): this;
  on(
    event: LifecycleEvent,
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): this {
    this.registry.addHook({
      ring: RingEvent.Lifecycle,
      stage: "single",
      lifecycleTag: lifecycleTagFor(event),
      run: (payload: unknown): unknown =>
        (handler as (arg: unknown) => Promise<void> | void)(payload),
    });
    return this;
  }

  onTrapDeny(
    handler: (
      event: TrapDenyEvent,
      session: Session & Ctx,
    ) => Promise<void> | void,
  ): this {
    this.registry.addHook({
      ring: RingEvent.Lifecycle,
      stage: "single",
      lifecycleTag: "trap-deny",
      run: (payload: unknown): unknown => {
        const args = Array.isArray(payload)
          ? (payload as readonly unknown[])
          : [payload, undefined];
        return (handler as (a: unknown, b: unknown) => Promise<void> | void)(
          args[0],
          args[1],
        );
      },
    });
    return this;
  }

  registerCustomAction<P>(
    name: string,
    schema: Schema<P>,
    handler: (payload: P) => Promise<void> | void,
  ): this {
    this.registry.addHook({
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
    this.registry.addHook({
      ring: RingEvent.VsockService,
      stage: "single",
      vsockKind: kind,
      run: (payload: unknown): unknown =>
        handler(validateSchema(schema, payload)),
    });
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  fs(fn: (scope: FsScope<Ctx>) => FsScope<Ctx>): this {
    this.checkMounted(".fs");
    this.registry.fs(fn);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  net(fn: (scope: NetScope<Ctx>) => NetScope<Ctx>): this {
    this.checkMounted(".net");
    this.registry.net(fn);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  console(fn: (scope: ConsoleScope<Ctx>) => ConsoleScope<Ctx>): this {
    this.checkMounted(".console");
    this.registry.console(fn);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  exec(fn: (scope: ExecScope<Ctx>) => ExecScope<Ctx>): this {
    this.checkMounted(".exec");
    this.registry.exec(fn);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  host(fn: (scope: HostScope<Ctx>) => HostScope<Ctx>): this {
    this.checkMounted(".host");
    this.registry.host(fn);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  lifecycle(fn: (scope: LifecycleScope<Ctx>) => LifecycleScope<Ctx>): this {
    this.checkMounted(".lifecycle");
    this.registry.lifecycle(fn);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  telemetry(fn: (scope: TelemetryScope<Ctx>) => TelemetryScope<Ctx>): this {
    this.checkMounted(".telemetry");
    this.registry.telemetry(fn);
    return this;
  }

  onNetOutbound(handler: (frame: Uint8Array) => FilterAction): this {
    this.checkMounted(".onNetOutbound"); this.registry.onNetOutbound(handler); return this;
  }

  onNetInbound(handler: (frame: Uint8Array) => FilterAction): this {
    this.checkMounted(".onNetInbound"); this.registry.onNetInbound(handler); return this;
  }

  onFsLookup(handler: (path: string) => Verdict): this {
    this.checkMounted(".onFsLookup"); this.registry.onFsLookup(handler); return this;
  }

  onFsRead(handler: (path: string, buffer: Uint8Array) => Uint8Array): this {
    this.checkMounted(".onFsRead"); this.registry.onFsRead(handler); return this;
  }

  onFsWrite(handler: (path: string, buffer: Uint8Array, pid: number, ctx: TrapContext & Ctx) => FilterAction): this {
    this.checkMounted(".onFsWrite"); this.registry.onFsWrite(handler); return this;
  }

  onFsLock(handler: (path: string, flockCmd: number, flockType: number, pid: number) => Verdict): this {
    this.checkMounted(".onFsLock"); this.registry.onFsLock(handler); return this;
  }

  onConsoleInput(handler: (port: number, bytes: Uint8Array) => Promise<Uint8Array> | Uint8Array): this {
    this.checkMounted(".onConsoleInput"); this.registry.onConsoleInput(handler); return this;
  }

  onConsoleOutput(handler: (port: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array): this {
    this.checkMounted(".onConsoleOutput"); this.registry.onConsoleOutput(handler); return this;
  }

  onExecStdout(handler: (pid: number, execId: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array): this {
    this.checkMounted(".onExecStdout"); this.registry.onExecStdout(handler); return this;
  }

  onExecStderr(handler: (pid: number, execId: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array): this {
    this.checkMounted(".onExecStderr"); this.registry.onExecStderr(handler); return this;
  }

  onHeartbeat(handler: (event: HeartbeatEvent) => Promise<void> | void): this {
    this.checkMounted(".onHeartbeat"); this.registry.onHeartbeat(handler); return this;
  }

  onProcessExit(handler: (event: ProcessExitEvent) => Promise<void> | void): this {
    this.checkMounted(".onProcessExit"); this.registry.onProcessExit(handler); return this;
  }

  onTrap(handler: (event: TrapInfo, c: TrapContext & Ctx) => Promise<Verdict> | Verdict): this {
    this.checkMounted(".onTrap"); this.registry.onTrap(handler); return this;
  }

  onWatchdog(handler: (event: WatchdogEvent) => Promise<WatchdogVerdict> | WatchdogVerdict): this {
    this.checkMounted(".onWatchdog"); this.registry.onWatchdog(handler); return this;
  }

  onHostLsmExec(handler: (path: string, pid: number) => Verdict): this {
    this.checkMounted(".onHostLsmExec"); this.registry.onHostLsmExec(handler); return this;
  }

  onHostPressure(handler: (level: number) => Verdict): this {
    this.checkMounted(".onHostPressure"); this.registry.onHostPressure(handler); return this;
  }

  onHostKeyringAccess(handler: (key: string) => Verdict): this {
    this.checkMounted(".onHostKeyringAccess"); this.registry.onHostKeyringAccess(handler); return this;
  }

  onVmInit(handler: (ctx: LifecycleContext) => Promise<void> | void): this {
    this.checkMounted(".onVmInit"); this.registry.onVmInit(handler); return this;
  }

  onVmReady(handler: (ctx: LifecycleContext) => Promise<void> | void): this {
    this.checkMounted(".onVmReady"); this.registry.onVmReady(handler); return this;
  }

  onVmShutdown(handler: (ctx: LifecycleContext) => Promise<void> | void): this {
    this.checkMounted(".onVmShutdown"); this.registry.onVmShutdown(handler); return this;
  }

  onPreCommit(handler: (ctx: LifecycleContext) => Promise<void> | void): this {
    this.checkMounted(".onPreCommit"); this.registry.onPreCommit(handler); return this;
  }

  onPostCommit(handler: (ctx: LifecycleContext, committed: boolean) => Promise<void> | void): this {
    this.checkMounted(".onPostCommit"); this.registry.onPostCommit(handler); return this;
  }

  onHostPreempt(handler: (ctx: LifecycleContext) => Promise<void> | void): this {
    this.checkMounted(".onHostPreempt"); this.registry.onHostPreempt(handler); return this;
  }

  onMemoryPressure(handler: (availableMb: number, totalMb: number) => Promise<void> | void): this {
    this.checkMounted(".onMemoryPressure"); this.registry.onMemoryPressure(handler); return this;
  }

  // see: cans/03-sdk-architecture.md#TypeScript-SDK
  requires<Req extends string>(
    name?: Req,
  ): KelirBuilder<Name, Ctx, Deps | Req, Params>;
  requires<P extends KelirBuilder<any, any, any, any>>(
    plugin?: P,
  ): KelirBuilder<
    Name,
    Ctx & PluginContextOf<P>,
    Deps | (P extends KelirBuilder<infer N, any, any, any> ? N : never),
    Params
  >;
  requires<Req extends string | KelirBuilder<any, any, any, any>>(
    name?: string | KelirBuilder<any, any, any, any>,
  ): KelirBuilder<
    Name,
    Ctx & (Req extends KelirBuilder<infer N, infer C, any, any>
      ? (C extends Record<string, unknown> ? (string extends keyof C ? { readonly [K in N]: C } : (N extends keyof C ? C : { readonly [K in N]: C })) : { readonly [K in N]: C })
      : unknown),
    Deps | (Req extends KelirBuilder<infer N, any, any, any> ? N : Req extends string ? Req : never),
    Params
  > {
    this.checkMounted(".requires");
    if (typeof name === "string") {
      if (!this.registry.requiredDeps.includes(name)) {
        this.registry.requiredDeps.push(name);
      }
    } else if (name !== undefined) {
      if (!this.registry.requiredDeps.includes(name.name)) {
        this.registry.requiredDeps.push(name.name);
      }
    }
    return this as unknown as KelirBuilder<
      Name,
      Ctx & (Req extends KelirBuilder<infer N, infer C, any, any>
        ? (C extends Record<string, unknown> ? (string extends keyof C ? { readonly [K in N]: C } : (N extends keyof C ? C : { readonly [K in N]: C })) : { readonly [K in N]: C })
        : unknown),
      Deps | (Req extends KelirBuilder<infer N, any, any, any> ? N : Req extends string ? Req : never),
      Params
    >;
  }

  // see: cans/03-sdk-architecture.md#TypeScript-SDK
  derive<NewCtx>(
    fn: (c: Ctx) => NewCtx,
  ): KelirBuilder<Name, Ctx & NewCtx, Deps, Params> {
    this.checkMounted(".derive");
    this.registry.deriveFns.push(fn as (c: unknown) => unknown);
    return this as unknown as KelirBuilder<Name, Ctx & NewCtx, Deps, Params>;
  }

  // see: cans/03-sdk-architecture.md#TypeScript-SDK
  context<NewCtx>(
    fn: (c: Ctx) => NewCtx,
  ): KelirBuilder<Name, Ctx & NewCtx, Deps, Params> {
    this.checkMounted(".context");
    this.registry.contextFns.push(fn as (c: unknown) => unknown);
    return this as unknown as KelirBuilder<Name, Ctx & NewCtx, Deps, Params>;
  }

  // see: cans/artifacts/code/plugin-trait.rs#MountScope
  scope(scope: MountScope): this {
    this.checkMounted(".scope");
    this.registry.setScope(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#KelirPlugin-trait
  forkWith(
    fn: () => KelirBuilder<Name, Ctx, Deps, Params>,
  ): this {
    this.checkMounted(".forkWith");
    this.forkFn = fn;
    return this;
  }

  forkState(): KelirBuilder<Name, Ctx, Deps, Params> {
    if (this.forkFn !== undefined) {
      return this.forkFn();
    }
    return this.clone();
  }

  private clone(): KelirBuilder<Name, Ctx, Deps, Params> {
    const copy = this.mode === "plugin"
      ? new KelirBuilder<Name, Ctx, Deps, Params>(this.name, this.storedOptions)
      : new KelirBuilder<Name, Ctx, Deps, Params>(this.kelirConfig);
    copy.registry = this.registry.clone();
    copy.mounted = [...this.mounted];
    copy.pluginRegistrations = [...this.pluginRegistrations];
    copy.detachedFlag = this.detachedFlag;
    copy.terminalCfg = this.terminalCfg;
    copy.storageCfg = this.storageCfg;
    copy.wireCfg = this.wireCfg;
    copy.paramsSchema = this.paramsSchema;
    copy.forkFn = this.forkFn;
    return copy;
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
    this.checkMounted(".vsockService");
    this.registry.vsockEntries.push({
      port,
      service,
      schema,
      handler: (request: unknown): Promise<unknown> | unknown =>
        handler(validateSchema(schema, request)),
    });
    return this;
  }

  private validate(): void {
    for (const mounted of this.mounted) {
      mounted.validate();
    }
    this.registry.validate();
  }

  // see: cans/02-plugin-model.md#Foreign-plugin-proxy
  fuse(): FusedTrampoline {
    this.validate();
    return FusedTrampoline.fuse(this.registry.hooks);
  }

  private fusedDescriptor(): FusedTrampolineDescriptor {
    return this.fuse().describe();
  }

  private async loadNativeAddon(): Promise<NativeAddonBinding> {
    const specifier = "./native/kelir.node";
    try {
      // TODO (L3): ship the precompiled NAPI addon next to the dist bundle and verify its ABI version.
      const addon = (await import(specifier)) as unknown as {
        readonly default?: NativeAddonBinding;
      };
      return (addon.default ?? addon) as NativeAddonBinding;
    } catch {
      throw new EConfig(
        "kelir native NAPI addon is missing: install the precompiled @kelir/core native build (L3)",
      );
    }
  }

  async boot(): Promise<Session & Ctx> {
    if (this.mode === "plugin") {
      throw new EConfig(
        `cannot boot plugin ${this.name}: mount it on a host with .use()`,
      );
    }
    const descriptor = this.fusedDescriptor();
    const addon = await this.loadNativeAddon();
    const nativeBuilder = new addon.NativeKelirBuilder({
      vcpus: this.kelirConfig.vcpus,
      memoryMb: this.kelirConfig.memoryMb,
      kernel: this.kelirConfig.kernel,
      rootfsErofs: this.kelirConfig.rootfsErofs,
      tscLocked: this.kelirConfig.tscLocked,
      enableWatchdog: this.kelirConfig.enableWatchdog,
      driver: this.kelirConfig.driver,
    });
    // TODO (L3): forward pageSizeKb/deterministicClock/pinnedClocksource/preemptionGraceMs once the native constructor accepts them.
    if (this.terminalCfg) {
      // TODO (L3): tokenShield.onClamp and prompt actions stay TypeScript-side until threadsafe-function bridging lands.
      nativeBuilder.setTerminalConfig({
        cols: this.terminalCfg.cols,
        rows: this.terminalCfg.rows,
        dropOsc52: this.terminalCfg.dropOsc52,
        stripTitle: this.terminalCfg.stripTitle,
        tokenShieldMaxBytesTotal: this.terminalCfg.tokenShield?.maxBytesTotal,
        tokenShieldMaxLinesTotal: this.terminalCfg.tokenShield?.maxLinesTotal,
        tokenShieldClampStrategy: this.terminalCfg.tokenShield?.clampStrategy,
        streamEngine: this.terminalCfg.streamEngine,
      });
    }
    if (this.storageCfg) {
      // TODO (L3): snoop onWrite callbacks stay TypeScript-side until threadsafe-function bridging lands.
      nativeBuilder.setStorageConfig({
        idmapGuestUid: this.storageCfg.idmap?.guestUid,
        idmapHostUid: this.storageCfg.idmap?.hostUid,
        sharedCaches: this.storageCfg.sharedCaches as string[] | undefined,
        lockBroker: this.storageCfg.lockBroker,
        syntheticFiles: this.storageCfg.syntheticFiles?.map((file) => ({
          path: file.path,
          content: typeof file.content === "function" ? file.content() : file.content,
          mode: file.mode,
        })),
        maskRead: this.storageCfg.maskRead?.map((rule) => ({
          glob: rule.glob,
          pattern: rule.pattern.source,
          replace: rule.replace,
        })),
        encapsulation: this.storageCfg.encapsulation,
        scanEngine: this.storageCfg.scanEngine,
        carryoverBytes: this.storageCfg.carryoverBytes,
      });
    }
    if (this.wireCfg) {
      nativeBuilder.setWireConfig({
        sinkholeDnsExcept: this.wireCfg.sinkhole?.dnsExcept as string[] | undefined,
        sinkholeDropAll: this.wireCfg.sinkhole !== undefined ? true : undefined,
        sshAgentBridge: this.wireCfg.sshAgentBridge,
        vaults: this.wireCfg.vaults,
        l4Proxies: this.wireCfg.l4Proxies?.map((proxy) => ({
          listenPort: proxy.listenPort,
          destination: proxy.destination,
          injectMtlsCertPath: proxy.injectMtls?.certPath,
          injectMtlsKeyPath: proxy.injectMtls?.keyPath,
        })),
        tapFilter: this.wireCfg.tapFilter,
      });
    }
    // TODO (L3): the params schema stays TypeScript-side; the native params model bridge lands with typed context threading.
    if (this.paramsSchema) {
      // TODO (L3): forward the pinned params schema once the native params model bridge exists.
    }
    for (const registration of this.pluginRegistrations) {
      nativeBuilder.addPlugin(registration.name, registration.enabled);
    }
    nativeBuilder.fuseHooks(descriptor.rings);
    if (nativeBuilder.trampolineEntryCount() !== descriptor.rings.length) {
      throw new EConfig(
        "kelir native NAPI trampoline table is not one entry per ring event",
      );
    }
    // TODO (L3): bind each fused ring chain to its single trampoline entry as one threadsafe closure.
    const nativeSession = await nativeBuilder.boot();
    const session = new Session(nativeSession);
    // see: cans/03-sdk-architecture.md#Service-lookup-across-languages
    const pluginContexts = nativeSession.pluginContexts;
    if (pluginContexts === undefined && this.pluginRegistrations.length > 0) {
      throw new EConfig(
        "kelir native bridge is missing the pluginContexts member: mounted plugin capability structs cannot be materialized at boot (B13)",
      );
    }
    for (const { name, ctx } of pluginContexts ?? []) {
      (session as unknown as Record<string, unknown>)[name] = ctx;
    }
    return session as Session & Ctx;
  }
}

export function kelir(config: KelirConfig = {}): KelirBuilder {
  return new KelirBuilder(config);
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
export function createPlugin<
  Name extends string,
  Ctx = unknown,
  Opts = unknown,
  Deps extends string = never,
>(name: Name, options?: CreatePluginOptions<Opts> | Ctx): KelirBuilder<Name, Ctx, Deps, unknown> {
  return new KelirBuilder<Name, Ctx, Deps, unknown>(name, options);
}
