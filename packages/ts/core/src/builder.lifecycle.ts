// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/02-plugin-model.md#Foreign-plugin-proxy
// see: cans/artifacts/code/typescript-accumulation.ts#runtime

import type { Schema, Infer } from "./schema.config.js";
import { validateSchema } from "./schema.config.js";
import type { TrapContext, LifecycleContext, HandlerContext } from "./context.lifecycle.types.js";
import type { Buffer } from "node:buffer";
import { HookRegistry } from "./hook.registry.js";
import type {
  KelirPlugin,
  MountScope,
  VsockServiceEntry,
  ActionsScope,
  StreamsScope,
  VsockScope,
  ParamsScope,
  DenyScope,
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
  LifecycleEvent,
  DnsVerdict,
  Verdict,
  FilterAction,
} from "./verdict.protocol.js";
import type {
  DirEntry,
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
  loadNativeAddon,
  type FusedTrampolineDescriptor,
  type HookDescriptor,
  type TrapFilterSnapshot,
} from "./fusion.plugin.js";

import type {
  KelirConfig,
  FfiBoundaryProfile,
  L4ProxyConfig,
  TerminalStream,
  TerminalConfig,
  StorageConfig,
  WireConfig,
  WireScope,
  LockBrokerConfig,
  CreatePluginOptions,
} from "./builder.config.js";
export type {
  KelirConfig,
  FfiBoundaryProfile,
  L4ProxyConfig,
  TerminalStream,
  TerminalConfig,
  StorageConfig,
  WireConfig,
  WireScope,
  LockBrokerConfig,
  CreatePluginOptions,
};

import type {
  MountRecord,
  PluginContextOf,
  PluginRegistration,
  TrapHandler,
  TrapHandlers,
  ValidateDeps,
} from "./builder.types.js";
export type {
  PluginContextOf,
  TrapHandler,
  TrapHandlerResult,
  TrapHandlers,
  ValidateDeps,
} from "./builder.types.js";

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
  private mountRecords: MountRecord[] = [];
  private mountedFlag = false;
  private sealedFlag = false;
  private lockBrokerCfg: LockBrokerConfig | undefined;
  private forkFn?: () => KelirBuilder<Name, Ctx, Deps, Params>;
  private detachedFlag: boolean;
  private terminalCfg: TerminalConfig | undefined;
  private storageCfg: StorageConfig | undefined;
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

  // L3-GAP: [plugin_engine]: the engine dispatch reads these entries once register_interceptors wiring lands per cans/02-plugin-model.md#Static-dispatch.; slop: interceptors never registered, traps bypass filter chain
  get vsockServices(): readonly VsockServiceEntry[] {
    return [
      ...this.registry.vsockEntries,
      ...this.mounted.flatMap((registry) => registry.vsockEntries),
    ];
  }

  getScope(): MountScope | undefined {
    return this.registry.getScope();
  }

  private checkVerb(verb: string): void {
    if (this.sealedFlag) throw new EConfig(`builder is sealed: ${verb} after .seal()`);
    if (this.mountedFlag) throw new EConfig(`plugin ${this.name} is already mounted: ${verb} after .use()`);
  }

  private rejectHostOnly(verb: string): never {
    throw new EConfig(`plugin ${this.name} called host-only verb ${verb}`);
  }

  // see: cans/04-terminal-plane.md#Terminal-pipeline
  terminal(config: TerminalConfig): this {
    this.checkVerb(".terminal");
    if (this.mode === "plugin") this.rejectHostOnly(".terminal");
    this.terminalCfg = config;
    return this;
  }

  // see: cans/05-storage-plane.md#Workspace-commit
  storage<C = Ctx>(config: StorageConfig): KelirBuilder<Name, Ctx & C, Deps, Params> {
    this.checkVerb(".storage");
    if (this.mode === "plugin") this.rejectHostOnly(".storage");
    this.storageCfg = config;
    return this as unknown as KelirBuilder<Name, Ctx & C, Deps, Params>;
  }

  // see: cans/07-wire-plane.md#DNS-policy-filter
  wire(config: WireConfig): this;
  wire(fn: (scope: WireScope) => WireScope | void): this;
  wire(
    configOrFn: WireConfig | ((scope: WireScope) => WireScope | void),
  ): this {
    this.checkVerb(".wire");
    if (this.mode === "plugin") this.rejectHostOnly(".wire");
    if (typeof configOrFn === "function") {
      const diversions: {
        readonly listenPort: number;
        readonly forwardUnixSocket: string;
      }[] = [];
      const scope: WireScope = {
        divertPortToUnix(listenPort, unixSocket) {
          if (
            !Number.isInteger(listenPort) ||
            listenPort < 1 ||
            listenPort > 65535
          ) {
            throw new EConfig(
              `divertPortToUnix port ${String(listenPort)} outside u16`,
            );
          }
          diversions.push({ listenPort, forwardUnixSocket: unixSocket });
          return scope;
        },
      };
      configOrFn(scope);
      this.wireCfg = {
        ...(this.wireCfg ?? {}),
        l4Proxies: [...(this.wireCfg?.l4Proxies ?? []), ...diversions],
      };
      return this;
    }
    this.wireCfg = configOrFn;
    return this;
  }

  // see: cans/05-storage-plane.md#Host-inode-lock-broker
  lockBroker(config: LockBrokerConfig): this {
    this.checkVerb(".lockBroker");
    if (this.mode === "plugin") this.rejectHostOnly(".lockBroker");
    this.lockBrokerCfg = config;
    return this;
  }

  // see: cans/03-sdk-architecture.md#TypeScript-SDK
  params<S extends Schema<any>>(schema: S): KelirBuilder<Name, Ctx, Deps, Infer<S>>;
  params(fn: (scope: ParamsScope<Ctx, this>) => ParamsScope<Ctx, this> | void): this;
  params(
    schemaOrFn:
      | Schema<any>
      | ((
          scope: ParamsScope<Ctx, KelirBuilder<Name, Ctx, Deps, Params>>,
        ) =>
          | ParamsScope<Ctx, KelirBuilder<Name, Ctx, Deps, Params>>
          | void),
  ): KelirBuilder<Name, Ctx, Deps, unknown> {
    this.checkVerb(".params");
    if (this.mode === "plugin") this.rejectHostOnly(".params");
    if (typeof schemaOrFn === "function") {
      const builder = this;
      const scope: ParamsScope<Ctx, KelirBuilder<Name, Ctx, Deps, Params>> = {
        model<S extends Schema<any>>(
          schema: S,
        ): ParamsScope<Ctx, KelirBuilder<Name, Ctx, Deps, Params>> {
          builder.paramsSchema = schema;
          return scope;
        },
      };
      schemaOrFn(scope);
      return this as unknown as KelirBuilder<Name, Ctx, Deps, unknown>;
    }
    this.paramsSchema = schemaOrFn;
    return this as unknown as KelirBuilder<Name, Ctx, Deps, unknown>;
  }

  // see: cans/08-lifecycle.md#Detached-lifecycle
  detached(detached: boolean): this {
    this.checkVerb(".detached");
    if (this.mode === "plugin") this.rejectHostOnly(".detached");
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
    this.checkVerb(".use");
    if (
      !(plugin instanceof KelirBuilder) &&
      (typeof plugin !== "object" ||
        plugin === null ||
        typeof (plugin as KelirPlugin).name !== "string")
    ) {
      throw new EConfig(
        `.use() expects a plugin builder or a KelirPlugin, got ${plugin === null ? "null" : typeof plugin}`,
      );
    }
    if (plugin instanceof KelirBuilder) {
      if (plugin.mode === "host") {
        throw new EConfig("cannot mount a host-mode builder as a plugin");
      }
      const options = plugin.storedOptions as
        | { readonly enabled?: boolean; readonly config?: unknown }
        | undefined;
      const enabled = options?.enabled !== false;
      const duplicate =
        plugin.mountedFlag ||
        this.pluginRegistrations.some(
          (registration) => registration.name === plugin.name,
        );
      this.mountRecords.push({
        name: plugin.name,
        enabled,
        requiredDeps: [...plugin.registry.requiredDeps],
        duplicate,
      });
      if (duplicate) {
        return this as unknown as KelirBuilder<Name, Ctx & PluginContextOf<P>, Deps, Params>;
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
    const duplicate = this.pluginRegistrations.some(
      (registration) => registration.name === asPlugin.name,
    );
    this.mountRecords.push({
      name: asPlugin.name,
      enabled,
      requiredDeps: [...(trait.requiredDeps ?? [])],
      duplicate,
    });
    if (duplicate) {
      return this as unknown as KelirBuilder<Name, Ctx & PluginContextOf<P>, Deps, Params>;
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
    this.checkVerb(".plugin");
    return this.use(plugin);
  }

  // see: cans/03-sdk-architecture.md#TypeScript-SDK
  pluginInstance<P extends KelirBuilder<any, any, any, any> | KelirPlugin>(
    plugin: P & ValidateDeps<Ctx, P>,
  ): KelirBuilder<Name, Ctx & PluginContextOf<P>, Deps, Params> {
    this.checkVerb(".pluginInstance");
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
  trap(fn: (scope: TrapScope<Ctx, this>) => TrapScope<Ctx, this> | void): this;
  // see: cans/02-plugin-model.md#Foreign-plugin-proxy
  trap(handlers: TrapHandlers<Ctx, Params>): this;
  trap(
    filterOrFnOrHandlers: string | TrapFilter | TrapHandlers<Ctx, Params> | ((scope: TrapScope<Ctx, this>) => TrapScope<Ctx, this> | void),
    handler?: TrapHandler<Ctx, Params> | TrapHandlers<Ctx, Params>,
  ): this {
    this.checkVerb(".trap");
    if (typeof filterOrFnOrHandlers === "function") {
      this.registry.trap(filterOrFnOrHandlers, () => this);
      return this;
    }
    if (typeof filterOrFnOrHandlers === "object" && filterOrFnOrHandlers !== null && typeof (filterOrFnOrHandlers as TrapFilter).snapshot !== "function") {
      return this.registerTrapHandlers(filterOrFnOrHandlers as TrapHandlers<Ctx, Params>, undefined);
    }
    const filterSnapshot =
      typeof filterOrFnOrHandlers === "string" ? TrapFilter.pattern(filterOrFnOrHandlers).snapshot() : (filterOrFnOrHandlers as TrapFilter).snapshot();
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
    return this.registerTrapHandlers(handler, filterSnapshot);
  }

  private registerTrapHandlers(handlers: TrapHandlers<Ctx, Params>, filter: TrapFilterSnapshot | undefined): this {
    if (handlers.pre !== undefined) {
      const pre = handlers.pre;
      this.registry.addHook({ ring: RingEvent.Trap, stage: "pre", filter, run: (event: unknown): unknown => pre(this.checkedTrapContext(event)) });
    }
    if (handlers.post !== undefined) {
      const post = handlers.post;
      this.registry.addHook({ ring: RingEvent.Trap, stage: "post", filter, run: (event: unknown): unknown => post(this.checkedTrapContext(event)) });
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
    this.checkVerb(".on");
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
    this.checkVerb(".onTrapDeny");
    this.registry.addHook({
      ring: RingEvent.Lifecycle,
      stage: "single",
      lifecycleTag: "trap-deny",
      run: (payload: unknown): unknown => {
        const args = Array.isArray(payload)
          ? (payload as readonly unknown[])
          : [payload, undefined];
        return Promise.resolve(
          (handler as (a: unknown, b: unknown) => Promise<void> | void)(
            args[0],
            args[1],
          ),
        ).catch((error: unknown): never => {
          throw EConfig.fromNative(error);
        });
      },
    });
    return this;
  }

  registerCustomAction<P>(
    name: string,
    schema: Schema<P>,
    handler: (payload: P) => Promise<void> | void,
  ): this {
    this.checkVerb(".registerCustomAction");
    this.registry.registerCustomAction(name, schema, handler);
    return this;
  }

  // see: cans/11-protocol.md#Control-channel
  registerStreamKind<Req, Resp = unknown>(
    kind: number,
    schema: Schema<Req>,
    handler: (request: Req) => Promise<Resp> | Resp,
  ): this {
    this.checkVerb(".registerStreamKind");
    this.registry.registerStreamKind(kind, schema, handler);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  fs(fn: (scope: FsScope<Ctx, this>) => FsScope<Ctx, this> | void): this {
    this.checkVerb(".fs");
    this.registry.fs(fn, () => this, this.mode === "plugin");
    const forkFn = this.registry.fsScopeForkFn;
    if (forkFn !== undefined) {
      this.forkFn = forkFn;
    }
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  net(fn: (scope: NetScope<Ctx, this>) => NetScope<Ctx, this> | void): this {
    this.checkVerb(".net");
    this.registry.net(fn, () => this);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  console(fn: (scope: ConsoleScope<Ctx, this>) => ConsoleScope<Ctx, this> | void): this {
    this.checkVerb(".console");
    this.registry.console(fn, () => this);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  exec(fn: (scope: ExecScope<Ctx, this>) => ExecScope<Ctx, this> | void): this {
    this.checkVerb(".exec");
    this.registry.exec(fn, () => this);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  host(fn: (scope: HostScope<Ctx, this>) => HostScope<Ctx, this> | void): this {
    this.checkVerb(".host");
    this.registry.host(fn, () => this);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  lifecycle(fn: (scope: LifecycleScope<Ctx, this>) => LifecycleScope<Ctx, this> | void): this {
    this.checkVerb(".lifecycle");
    this.registry.lifecycle(fn, () => this);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  telemetry(fn: (scope: TelemetryScope<Ctx, this>) => TelemetryScope<Ctx, this> | void): this {
    this.checkVerb(".telemetry");
    this.registry.telemetry(fn, () => this);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  actions(fn: (scope: ActionsScope<Ctx, this>) => ActionsScope<Ctx, this> | void): this {
    this.checkVerb(".actions");
    this.registry.actions(fn);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  streams(fn: (scope: StreamsScope<Ctx, this>) => StreamsScope<Ctx, this> | void): this {
    this.checkVerb(".streams");
    this.registry.streams(fn);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  vsock(fn: (scope: VsockScope<Ctx, this>) => VsockScope<Ctx, this> | void): this {
    this.checkVerb(".vsock");
    this.registry.vsock(fn);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  deny(fn: (scope: DenyScope<Ctx, this>) => DenyScope<Ctx, this> | void): this {
    this.checkVerb(".deny");
    this.registry.deny(fn);
    return this;
  }

  onNetOutbound(handler: (frame: Uint8Array) => FilterAction): this {
    this.checkVerb(".onNetOutbound"); this.registry.onNetOutbound(handler); return this;
  }

  onNetInbound(handler: (frame: Uint8Array) => FilterAction): this {
    this.checkVerb(".onNetInbound"); this.registry.onNetInbound(handler); return this;
  }

  onSocketConnect(handler: (pid: number, proto: number, ip: readonly [number, number, number, number], port: number) => Verdict): this {
    this.checkVerb(".onSocketConnect"); this.registry.onSocketConnect(handler); return this;
  }

  onDnsQuery(handler: (pid: number, domain: string, qtype: number) => DnsVerdict): this {
    this.checkVerb(".onDnsQuery"); this.registry.onDnsQuery(handler); return this;
  }

  onSocketBind(handler: (pid: number, proto: number, ip: readonly [number, number, number, number], port: number) => Verdict): this {
    this.checkVerb(".onSocketBind"); this.registry.onSocketBind(handler); return this;
  }

  onFsLookup(handler: (path: string) => Verdict): this {
    this.checkVerb(".onFsLookup"); this.registry.onFsLookup(handler); return this;
  }

  onFsRead(handler: (path: string, buffer: Uint8Array) => Uint8Array): this {
    this.checkVerb(".onFsRead"); this.registry.onFsRead(handler); return this;
  }

  onFsWrite(handler: (path: string, buffer: Buffer, pid: number, ctx: TrapContext & Ctx) => FilterAction): this {
    this.checkVerb(".onFsWrite"); this.registry.onFsWrite(handler); return this;
  }

  onFsLock(handler: (path: string, flockCmd: number, flockType: number, pid: number) => Verdict): this {
    this.checkVerb(".onFsLock"); this.registry.onFsLock(handler); return this;
  }

  onFsUnlink(handler: (path: string, ctx: TrapContext & Ctx) => Verdict): this {
    this.checkVerb(".onFsUnlink"); this.registry.onFsUnlink(handler); return this;
  }

  onFsRename(handler: (src: string, dst: string, ctx: TrapContext & Ctx) => Verdict): this {
    this.checkVerb(".onFsRename"); this.registry.onFsRename(handler); return this;
  }

  onFsSetattr(handler: (path: string, mode: number, size: number) => Verdict): this {
    this.checkVerb(".onFsSetattr"); this.registry.onFsSetattr(handler); return this;
  }

  onFsSymlink(handler: (target: string, link: string) => Verdict): this {
    this.checkVerb(".onFsSymlink"); this.registry.onFsSymlink(handler); return this;
  }

  onFsMkdir(handler: (path: string, mode: number) => Verdict): this {
    this.checkVerb(".onFsMkdir"); this.registry.onFsMkdir(handler); return this;
  }

  onFsReaddir(handler: (path: string, entries: DirEntry[]) => FilterAction): this {
    this.checkVerb(".onFsReaddir"); this.registry.onFsReaddir(handler); return this;
  }

  onFsReadlink(handler: (path: string) => Verdict): this {
    this.checkVerb(".onFsReadlink"); this.registry.onFsReadlink(handler); return this;
  }

  onConsoleInput(handler: (port: number, bytes: Uint8Array) => Promise<Uint8Array> | Uint8Array): this {
    this.checkVerb(".onConsoleInput"); this.registry.onConsoleInput(handler); return this;
  }

  onConsoleOutput(handler: (port: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array): this {
    this.checkVerb(".onConsoleOutput"); this.registry.onConsoleOutput(handler); return this;
  }

  onExecStdout(handler: (pid: number, execId: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array): this {
    this.checkVerb(".onExecStdout"); this.registry.onExecStdout(handler); return this;
  }

  onExecStderr(handler: (pid: number, execId: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array): this {
    this.checkVerb(".onExecStderr"); this.registry.onExecStderr(handler); return this;
  }

  onExecStdin(handler: (pid: number, execId: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array): this {
    this.checkVerb(".onExecStdin"); this.registry.onExecStdin(handler); return this;
  }

  onHeartbeat(handler: (event: HeartbeatEvent) => Promise<void> | void): this {
    this.checkVerb(".onHeartbeat"); this.registry.onHeartbeat(handler); return this;
  }

  onProcessExit(handler: (event: ProcessExitEvent) => Promise<void> | void): this {
    this.checkVerb(".onProcessExit"); this.registry.onProcessExit(handler); return this;
  }

  onSignal(handler: (sender: number, target: number, sig: number) => Promise<Verdict> | Verdict): this {
    this.checkVerb(".onSignal"); this.registry.onSignal(handler); return this;
  }

  onFork(handler: (parent: number, child: number) => Promise<Verdict> | Verdict): this {
    this.checkVerb(".onFork"); this.registry.onFork(handler); return this;
  }

  onTrap(handler: (event: TrapInfo, c: TrapContext & Ctx) => Promise<Verdict> | Verdict): this {
    this.checkVerb(".onTrap"); this.registry.onTrap(handler); return this;
  }

  onWatchdog(handler: (event: WatchdogEvent) => Promise<WatchdogVerdict> | WatchdogVerdict): this {
    this.checkVerb(".onWatchdog"); this.registry.onWatchdog(handler); return this;
  }

  onHostLsmExec(handler: (path: string, pid: number) => Verdict): this {
    this.checkVerb(".onHostLsmExec"); this.registry.onHostLsmExec(handler); return this;
  }

  onHostPressure(handler: (level: number) => Verdict): this {
    this.checkVerb(".onHostPressure"); this.registry.onHostPressure(handler); return this;
  }

  onHostKeyringAccess(handler: (key: string) => Verdict): this {
    this.checkVerb(".onHostKeyringAccess"); this.registry.onHostKeyringAccess(handler); return this;
  }

  onVmInit(handler: (ctx: LifecycleContext) => Promise<void> | void): this {
    this.checkVerb(".onVmInit"); this.registry.onVmInit(handler); return this;
  }

  onVmReady(handler: (ctx: LifecycleContext) => Promise<void> | void): this {
    this.checkVerb(".onVmReady"); this.registry.onVmReady(handler); return this;
  }

  onVmShutdown(handler: (ctx: LifecycleContext) => Promise<void> | void): this {
    this.checkVerb(".onVmShutdown"); this.registry.onVmShutdown(handler); return this;
  }

  onPreCommit(handler: (ctx: LifecycleContext) => Promise<void> | void): this {
    this.checkVerb(".onPreCommit"); this.registry.onPreCommit(handler); return this;
  }

  onPostCommit(handler: (ctx: LifecycleContext, committed: boolean) => Promise<void> | void): this {
    this.checkVerb(".onPostCommit"); this.registry.onPostCommit(handler); return this;
  }

  onHostPreempt(handler: (ctx: LifecycleContext) => Promise<void> | void): this {
    this.checkVerb(".onHostPreempt"); this.registry.onHostPreempt(handler); return this;
  }

  onHostContainmentBreach(handler: (pid: number, path: string, kind: string) => Promise<void> | void): this {
    this.checkVerb(".onHostContainmentBreach"); this.registry.onHostContainmentBreach(handler); return this;
  }

  onMemoryPressure(handler: (availableMb: number, totalMb: number) => Promise<void> | void): this {
    this.checkVerb(".onMemoryPressure"); this.registry.onMemoryPressure(handler); return this;
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
    this.checkVerb(".requires");
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
    this.checkVerb(".derive");
    this.registry.deriveFns.push(fn as (c: unknown) => unknown);
    return this as unknown as KelirBuilder<Name, Ctx & NewCtx, Deps, Params>;
  }

  // see: cans/03-sdk-architecture.md#TypeScript-SDK
  context<NewCtx>(
    fn: (c: Ctx) => NewCtx,
  ): KelirBuilder<Name, Ctx & NewCtx, Deps, Params> {
    this.checkVerb(".context");
    this.registry.contextFns.push(fn as (c: unknown) => unknown);
    return this as unknown as KelirBuilder<Name, Ctx & NewCtx, Deps, Params>;
  }

  // see: cans/artifacts/code/plugin-trait.rs#MountScope
  scope(scope: MountScope): this {
    this.checkVerb(".scope");
    this.registry.setScope(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  group(fn: (builder: this) => this | void): this {
    this.checkVerb(".group");
    const result = fn(this);
    if (result !== undefined && result !== this) throw new EConfig("group() must return the builder it receives");
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  guard(enabled: boolean, fn: (builder: this) => this | void): this {
    this.checkVerb(".guard");
    if (!enabled) return this;
    const result = fn(this);
    if (result !== undefined && result !== this) throw new EConfig("guard() must return the builder it receives");
    return this;
  }

  // see: cans/02-plugin-model.md#KelirPlugin-trait
  forkWith(
    fn: () => KelirBuilder<Name, Ctx, Deps, Params>,
  ): this {
    this.checkVerb(".forkWith");
    this.forkFn = fn;
    return this;
  }

  forkState(): KelirBuilder<Name, Ctx, Deps, Params> {
    if (this.mode === "host") {
      throw new EConfig(
        "host-mode KelirBuilder forks via Session.fork(), not .forkState()",
      );
    }
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
    copy.mountRecords = [...this.mountRecords];
    copy.detachedFlag = this.detachedFlag;
    copy.terminalCfg = this.terminalCfg;
    copy.storageCfg = this.storageCfg;
    copy.wireCfg = this.wireCfg;
    copy.lockBrokerCfg = this.lockBrokerCfg;
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
    this.checkVerb(".vsockService");
    this.registry.vsockService(port, service, schema, handler);
    return this;
  }

  private validate(): void {
    for (const mounted of this.mounted) {
      mounted.validate();
    }
    this.registry.validate();
  }

  private validateMounts(): void {
    const seen = new Set<string>();
    for (const record of this.mountRecords) {
      if (record.duplicate || seen.has(record.name)) {
        throw new EConfig(`plugin already mounted: ${record.name}`);
      }
      seen.add(record.name);
    }
    const mounted = new Set(
      this.pluginRegistrations.map((registration) => registration.name),
    );
    for (const record of this.mountRecords) {
      if (!record.enabled) continue;
      for (const dep of record.requiredDeps) {
        if (!mounted.has(dep)) {
          throw new EConfig(
            `missing prerequisite: ${dep} required by ${record.name}`,
          );
        }
      }
    }
  }

  // see: cans/02-plugin-model.md#One-validation,-at-seal-and-boot
  seal(): this {
    if (!this.sealedFlag) {
      this.validate();
      this.validateMounts();
      this.sealedFlag = true;
    }
    return this;
  }

  hooksForFusion(): FusedTrampolineDescriptor {
    this.validate();
    return FusedTrampoline.fuse(this.registry.hooks).describe();
  }

  // see: cans/02-plugin-model.md#Foreign-plugin-proxy
  fuse(): FusedTrampoline {
    this.seal();
    return FusedTrampoline.fuse(this.registry.hooks);
  }

  async boot(): Promise<Session & Ctx> {
    if (this.mode === "plugin") {
      throw new EConfig(
        `cannot boot plugin ${this.name}: mount it on a host with .use()`,
      );
    }
    const fused = this.fuse();
    const descriptor = fused.describe();
    const addon = await loadNativeAddon();
    const nativeBuilder = new addon.NativeKelirBuilder({
      vcpus: this.kelirConfig.vcpus,
      memoryMb: this.kelirConfig.memoryMb,
      kernel: this.kelirConfig.kernel,
      rootfsErofs: this.kelirConfig.rootfsErofs,
      tscLocked: this.kelirConfig.tscLocked,
      enableWatchdog: this.kelirConfig.enableWatchdog,
      driver: this.kelirConfig.driver,
      networkOff: this.kelirConfig.networkOff,
      nsPerInstruction: this.kelirConfig.nsPerInstruction,
      entropySeed: this.kelirConfig.entropySeed,
    });
    // L3-GAP: [bindings_ts]: forward pageSizeKb/deterministicClock/pinnedClocksource/preemptionGraceMs once the native constructor accepts them.; slop: boot flags silently dropped by native constructor
    // TODO (L3) [bindings_ts]: forward ffiBoundary.embedMode once the native constructor accepts it; the embed mode stays TypeScript-side until then; slop: embed mode stays TypeScript-side; the native constructor never sees it.
    if (this.terminalCfg) {
      nativeBuilder.setTerminalConfig({
        cols: this.terminalCfg.cols,
        rows: this.terminalCfg.rows,
        dropOsc52: this.terminalCfg.dropOsc52,
        stripTitle: this.terminalCfg.stripTitle,
        streamEngine: this.terminalCfg.streamEngine,
      });
    }
    if (this.storageCfg !== undefined || this.lockBrokerCfg !== undefined) {
      nativeBuilder.setStorageConfig({
        idmapGuestUid: this.storageCfg?.idmap?.guestUid,
        idmapHostUid: this.storageCfg?.idmap?.hostUid,
        sharedCaches: this.storageCfg?.sharedCaches as string[] | undefined,
        lockBroker: this.storageCfg?.lockBroker ?? this.lockBrokerCfg !== undefined,
        lockBrokerGlobs: this.lockBrokerCfg?.globs as string[] | undefined,
        lockBrokerNonBlocking: this.lockBrokerCfg?.nonBlocking,
        encapsulation: this.storageCfg?.encapsulation,
      });
    }
    if (this.wireCfg) {
      // see: cans/07-wire-plane.md#Sinkhole-rules
      const explicitRules = this.wireCfg.sinkhole?.rules?.map((rule) => ({
        host: rule.host,
        allow: rule.allow,
      }));
      nativeBuilder.setWireConfig({
        sinkhole:
          explicitRules !== undefined ? { rules: explicitRules } : undefined,
        sshAgentBridge: this.wireCfg.sshAgentBridge,
        vaults: this.wireCfg.vaults,
        // TODO (L3) [wire_plane]: forward forwardUnixSocket diversions once the NAPI wire config accepts them; divert entries stay TypeScript-side until then; slop: unix-socket diversions stay TypeScript-side; the NAPI wire config never sees them.
        l4Proxies: this.wireCfg.l4Proxies
          ?.filter(
            (proxy): proxy is L4ProxyConfig & { readonly destination: string } =>
              proxy.destination !== undefined,
          )
          .map((proxy) => ({
            listenPort: proxy.listenPort,
            destination: proxy.destination,
            injectMtlsCertPath: proxy.injectMtls?.certPath,
            injectMtlsKeyPath: proxy.injectMtls?.keyPath,
          })),
        tapFilter: this.wireCfg.tapFilter,
      });
    }
    // L3-GAP: [bt_01_typescript_boot_econfig_missing_typebox_dependency]: the params schema stays TypeScript-side; the native params model bridge lands with typed context threading.; slop: params schema stays unvalidated stub, boot passes bad params
    if (this.paramsSchema) {
      // L3-GAP: [bt_01_typescript_boot_econfig_missing_typebox_dependency]: forward the pinned params schema once the native params model bridge exists.; slop: pinned params schema never forwarded, native validates nothing
    }
    for (const registration of this.pluginRegistrations) {
      nativeBuilder.addPlugin(registration.name, registration.enabled);
    }
    nativeBuilder.fuseHooks(descriptor.rings, fused.runs());
    if (nativeBuilder.trampolineEntryCount() !== descriptor.rings.length) {
      throw new EConfig(
        "kelir native NAPI trampoline table is not one entry per ring event",
      );
    }
    // L3-GAP: [bt_03_typescript_fused_trampoline_single_crossing_chained_hooks]: rendezvous the JS return value into a Verdict and marshal the ring event payload across the threadsafe boundary; slop: JS hook runs fire-and-forget, its verdict never gates dispatch
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
export function createPlugin<Name extends string, Ctx = unknown, Opts = unknown, Deps extends string = never, Params = unknown>(
  name: Name, options?: CreatePluginOptions<Opts> | Ctx,
): KelirBuilder<Name, Ctx, Deps, Params> {
  return new KelirBuilder<Name, Ctx, Deps, Params>(name, options);
}
