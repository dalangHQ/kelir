// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/02-plugin-model.md#Foreign-plugin-proxy
// see: cans/02-plugin-model.md#Host-only-verbs
// see: cans/artifacts/code/typescript-accumulation.ts#runtime

import type { Schema } from "./schema.config.js";
import { validateSchema } from "./schema.config.js";
import type { HandlerContext } from "./context.lifecycle.types.js";
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
import { Session } from "./session.lifecycle.js";
import { EConfig } from "./errors.sdk.js";
import { TrapFilter } from "./trap.protocol.js";
import {
  FusedTrampoline,
  RingEvent,
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
import {
  checkpointConfigFromProfile,
  configFromProfile,
  hostConfigFromProfile,
  readProfile,
  storageConfigFromProfile,
  terminalConfigFromProfile,
  wireConfigFromProfile,
  type CheckpointPlaneConfig,
  type HostPlaneConfig,
  type ProfileSpec,
} from "./profile.schema.js";
export type {
  CheckpointPlaneConfig,
  HostPlaneConfig,
  ProfileSpec,
} from "./profile.schema.js";

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
// see: cans/02-plugin-model.md#Host-only-verbs
export type BuilderMode = "host" | "plugin";
export type BuilderSealState = "unsealed" | "sealed";
type AnyBuilder = KelirBuilder<BuilderMode, BuilderSealState, string, unknown, string, unknown>;

export class KelirBuilder<
  TMode extends BuilderMode = BuilderMode,
  TSealed extends BuilderSealState = BuilderSealState,
  Name extends string = string,
  Ctx = unknown,
  Deps extends string = never,
  Params = unknown,
> {
  // see: cans/02-plugin-model.md#Host-only-verbs
  private readonly __mode: TMode;
  private readonly __seal: TSealed;
  private readonly mode: BuilderMode;
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
  private forkFn?: () => AnyBuilder;
  private detachedFlag: boolean;
  private terminalCfg: TerminalConfig | undefined;
  private storageCfg: StorageConfig | undefined;
  private wireCfg: WireConfig | undefined;
  private hostCfg: HostPlaneConfig | undefined;
  private checkpointCfg: CheckpointPlaneConfig | undefined;
  private profileSpec: ProfileSpec | undefined;
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
    this.__mode = this.mode as TMode;
    this.__seal = "unsealed" as TSealed;
    void this.__mode;
    void this.__seal;
  }

  // see: cans/plugins/profile.md#Declarative-profile-execution
  // see: cans/plugins/profile.md#Inversion-of-Control-configuration-router
  static async fromConfig(path: string): Promise<KelirBuilder<"host", "unsealed">> {
    const profile = await readProfile(path);
    const builder = new KelirBuilder<"host", "unsealed">(configFromProfile(profile));
    builder.profileSpec = profile;
    const terminal = terminalConfigFromProfile(profile);
    const storage = storageConfigFromProfile(profile);
    const wire = wireConfigFromProfile(profile);
    const host = hostConfigFromProfile(profile);
    const checkpoint = checkpointConfigFromProfile(profile);
    if (terminal !== undefined) builder.terminal(terminal);
    if (storage !== undefined) builder.storage(storage);
    if (wire !== undefined) builder.wire(wire);
    if (host !== undefined) builder.host(host);
    if (checkpoint !== undefined) builder.checkpoint(checkpoint);
    return builder;
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

  private rejectPluginOnly(verb: string): never {
    throw new EConfig(`host ${this.name} called plugin-only verb ${verb}`);
  }

  // see: cans/04-terminal-plane.md#Terminal-pipeline
  terminal(
    this: KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>,
    config: TerminalConfig,
  ): KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params> {
    this.checkVerb(".terminal");
    if (this.mode === "plugin") this.rejectHostOnly(".terminal");
    this.terminalCfg = config;
    return this;
  }

  // see: cans/05-storage-plane.md#Workspace-commit
  storage<C = Ctx>(
    this: KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>,
    config: StorageConfig,
  ): KelirBuilder<TMode, TSealed, Name, Ctx & C, Deps, Params> {
    this.checkVerb(".storage");
    if (this.mode === "plugin") this.rejectHostOnly(".storage");
    if ("lockBroker" in config) {
      throw new EConfig(
        "storage() does not accept lockBroker: set it via .fs((f) => f.config({ lockBroker }))",
      );
    }
    this.storageCfg = config;
    return this as unknown as KelirBuilder<TMode, TSealed, Name, Ctx & C, Deps, Params>;
  }

  // see: cans/07-wire-plane.md#DNS-policy-filter
  wire(
    this: KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>,
    config: WireConfig,
  ): KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>;
  wire(
    this: KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>,
    fn: (scope: WireScope) => WireScope | void,
  ): KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>;
  wire(
    this: KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>,
    configOrFn: WireConfig | ((scope: WireScope) => WireScope | void),
  ): KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params> {
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

  // see: cans/03-sdk-architecture.md#TypeScript-SDK
  // see: cans/03-sdk-architecture.md#Params-typed-views
  params<Params2 = Params>(
    this: KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>,
    fn: (
      scope: ParamsScope<Ctx, KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>>,
    ) => ParamsScope<Ctx, KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>> | void,
  ): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, Params2> {
    this.checkVerb(".params");
    if (this.mode === "plugin") this.rejectHostOnly(".params");
    const builder = this;
    const scope: ParamsScope<Ctx, KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>> = {
      model<S extends Schema<any>>(
        schema: S,
      ): ParamsScope<Ctx, KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>> {
        builder.paramsSchema = schema;
        return scope;
      },
      // see: cans/02-plugin-model.md#Nested-scopes
      requires<Req extends string>(
        name?: Req,
      ): ParamsScope<Ctx, KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>> {
        if (name !== undefined && !builder.registry.requiredDeps.includes(name)) {
          builder.registry.requiredDeps.push(name);
        }
        return scope;
      },
      timeout(
        seconds: number,
      ): ParamsScope<Ctx, KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>> {
        builder.registry.scopeTimeoutSeconds["params"] = seconds;
        return scope;
      },
      priority(
        i: number,
      ): ParamsScope<Ctx, KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>> {
        builder.registry.scopePriorities["params"] = i;
        return scope;
      },
    };
    fn(scope);
    return this as unknown as KelirBuilder<TMode, TSealed, Name, Ctx, Deps, Params2>;
  }

  // see: cans/08-lifecycle.md#Detached-lifecycle
  detached(
    this: KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>,
    detached: boolean,
  ): KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params> {
    this.checkVerb(".detached");
    if (this.mode === "plugin") this.rejectHostOnly(".detached");
    this.detachedFlag = detached;
    return this;
  }

  isDetached(): boolean {
    return this.detachedFlag;
  }

  // see: cans/artifacts/code/typescript-accumulation.ts#runtime
  use<P extends AnyBuilder | KelirPlugin>(
    this: KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>,
    plugin: P & ValidateDeps<Ctx, P>,
  ): KelirBuilder<"host", "unsealed", Name, Ctx & PluginContextOf<P>, Deps, Params> {
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
    if (typeof plugin === "string") {
      throw new EConfig(
        `.use() expects a plugin builder or a KelirPlugin, got string`,
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
        return this as unknown as KelirBuilder<"host", "unsealed", Name, Ctx & PluginContextOf<P>, Deps, Params>;
      }
      for (const hook of plugin.registry.hooks) {
        this.registry.adoptHook(hook);
      }
      this.mounted.push(plugin.registry as HookRegistry<Ctx>);
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
      return this as unknown as KelirBuilder<"host", "unsealed", Name, Ctx & PluginContextOf<P>, Deps, Params>;
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
      return this as unknown as KelirBuilder<"host", "unsealed", Name, Ctx & PluginContextOf<P>, Deps, Params>;
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
    return this as unknown as KelirBuilder<"host", "unsealed", Name, Ctx & PluginContextOf<P>, Deps, Params>;
  }

  // see: cans/02-plugin-model.md#Unified-builder
  plugin<P extends AnyBuilder | KelirPlugin>(
    this: KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>,
    plugin: P & ValidateDeps<Ctx, P>,
  ): KelirBuilder<"host", "unsealed", Name, Ctx & PluginContextOf<P>, Deps, Params> {
    this.checkVerb(".plugin");
    if (typeof plugin === "string") {
      throw new EConfig(
        `.plugin() expects a plugin builder or a KelirPlugin, got string`,
      );
    }
    return this.use(plugin);
  }

  // see: cans/03-sdk-architecture.md#TypeScript-SDK
  pluginInstance<P extends AnyBuilder | KelirPlugin>(
    this: KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>,
    plugin: P & ValidateDeps<Ctx, P>,
  ): KelirBuilder<"host", "unsealed", Name, Ctx & PluginContextOf<P>, Deps, Params> {
    this.checkVerb(".pluginInstance");
    if (typeof plugin === "string") {
      throw new EConfig(
        `.pluginInstance() expects a plugin builder or a KelirPlugin, got string`,
      );
    }
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
    this: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>,
    filter: string | TrapFilter,
    handler: TrapHandler<Ctx, Params> | TrapHandlers<Ctx, Params>,
  ): KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>;
  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  trap(
    this: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>,
    fn: (scope: TrapScope<Ctx, KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>>) => TrapScope<Ctx, KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>> | void,
  ): KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>;
  // see: cans/02-plugin-model.md#Foreign-plugin-proxy
  trap(
    this: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>,
    handlers: TrapHandlers<Ctx, Params>,
  ): KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>;
  trap(
    this: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>,
    filterOrFnOrHandlers: string | TrapFilter | TrapHandlers<Ctx, Params> | ((scope: TrapScope<Ctx, KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>>) => TrapScope<Ctx, KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>> | void),
    handler?: TrapHandler<Ctx, Params> | TrapHandlers<Ctx, Params>,
  ): KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params> {
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

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  fs(
    this: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>,
    fn: (scope: FsScope<Ctx, KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>>) => FsScope<Ctx, KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>> | void,
  ): KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params> {    this.checkVerb(".fs");
    this.registry.fs(fn, () => this, this.mode === "plugin");
    const forkFn = this.registry.fsScopeForkFn;
    if (forkFn !== undefined) {
      this.forkFn = forkFn;
    }
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  net(
    this: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>,
    fn: (scope: NetScope<Ctx, KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>>) => NetScope<Ctx, KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>> | void,
  ): KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params> {
    this.checkVerb(".net");
    this.registry.net(fn, () => this);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  console(
    this: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>,
    fn: (scope: ConsoleScope<Ctx, KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>>) => ConsoleScope<Ctx, KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>> | void,
  ): KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params> {
    this.checkVerb(".console");
    this.registry.console(fn, () => this);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  exec(
    this: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>,
    fn: (scope: ExecScope<Ctx, KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>>) => ExecScope<Ctx, KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>> | void,
  ): KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params> {
    this.checkVerb(".exec");
    this.registry.exec(fn, () => this);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  host(
    this: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>,
    config: HostPlaneConfig,
  ): KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>;
  host(
    this: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>,
    fn: (scope: HostScope<Ctx, KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>>) => HostScope<Ctx, KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>> | void,
  ): KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>;
  host(
    this: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>,
    configOrFn:
      | HostPlaneConfig
      | ((scope: HostScope<Ctx, KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>>) => HostScope<Ctx, KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>> | void),
  ): KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params> {
    this.checkVerb(".host");
    if (typeof configOrFn === "function") {
      this.registry.host(configOrFn, () => this);
      return this;
    }
    if (this.mode === "plugin") this.rejectHostOnly(".host");
    this.hostCfg = configOrFn;
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  checkpoint(
    this: KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>,
    config: CheckpointPlaneConfig,
  ): KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params> {
    this.checkVerb(".checkpoint");
    if (this.mode === "plugin") this.rejectHostOnly(".checkpoint");
    this.checkpointCfg = config;
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  lifecycle(
    this: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>,
    fn: (scope: LifecycleScope<Ctx, KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>>) => LifecycleScope<Ctx, KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>> | void,
  ): KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params> {
    this.checkVerb(".lifecycle");
    this.registry.lifecycle(fn, () => this);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  telemetry(
    this: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>,
    fn: (scope: TelemetryScope<Ctx, KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>>) => TelemetryScope<Ctx, KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>> | void,
  ): KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params> {
    this.checkVerb(".telemetry");
    this.registry.telemetry(fn, () => this);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  actions(
    this: KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>,
    fn: (scope: ActionsScope<Ctx, KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>>) => ActionsScope<Ctx, KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>> | void,
  ): KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params> {
    this.checkVerb(".actions");
    if (this.mode === "plugin") this.rejectHostOnly(".actions");
    this.registry.actions(fn);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  streams(
    this: KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>,
    fn: (scope: StreamsScope<Ctx, KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>>) => StreamsScope<Ctx, KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>> | void,
  ): KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params> {
    this.checkVerb(".streams");
    if (this.mode === "plugin") this.rejectHostOnly(".streams");
    this.registry.streams(fn);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  vsock(
    this: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>,
    fn: (scope: VsockScope<Ctx, KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>>) => VsockScope<Ctx, KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>> | void,
  ): KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params> {
    this.checkVerb(".vsock");
    this.registry.vsock(fn);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  deny(
    this: KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>,
    fn: (scope: DenyScope<Ctx, KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>>) => DenyScope<Ctx, KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params>> | void,
  ): KelirBuilder<"host", "unsealed", Name, Ctx, Deps, Params> {
    this.checkVerb(".deny");
    if (this.mode === "plugin") this.rejectHostOnly(".deny");
    this.registry.deny(fn);
    return this;
  }

  // see: cans/03-sdk-architecture.md#TypeScript-SDK
  requires<Req extends string>(
    this: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>,
    name?: Req,
  ): KelirBuilder<TMode, "unsealed", Name, Ctx, Deps | Req, Params>;
  requires<P extends AnyBuilder>(
    this: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>,
    plugin?: P,
  ): KelirBuilder<
    TMode,
    "unsealed",
    Name,
    Ctx & PluginContextOf<P>,
    Deps | (P extends KelirBuilder<infer _M, infer _S, infer N, any, any, any> ? N : never),
    Params
  >;
  requires<Req extends string | AnyBuilder>(
    this: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>,
    name?: string | AnyBuilder,
  ): KelirBuilder<
    TMode,
    "unsealed",
    Name,
    Ctx & (Req extends KelirBuilder<infer _M, infer _S, infer N, infer C, any, any>
      ? (C extends Record<string, unknown> ? (string extends keyof C ? { readonly [K in N]: C } : (N extends keyof C ? C : { readonly [K in N]: C })) : { readonly [K in N]: C })
      : unknown),
    Deps | (Req extends KelirBuilder<infer _M, infer _S, infer N, any, any, any> ? N : Req extends string ? Req : never),
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
      TMode,
      "unsealed",
      Name,
      Ctx & (Req extends KelirBuilder<infer _M, infer _S, infer N, infer C, any, any>
        ? (C extends Record<string, unknown> ? (string extends keyof C ? { readonly [K in N]: C } : (N extends keyof C ? C : { readonly [K in N]: C })) : { readonly [K in N]: C })
        : unknown),
      Deps | (Req extends KelirBuilder<infer _M, infer _S, infer N, any, any, any> ? N : Req extends string ? Req : never),
      Params
    >;
  }

  // see: cans/03-sdk-architecture.md#TypeScript-SDK
  derive<NewCtx>(
    this: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>,
    fn: (c: Ctx) => NewCtx,
  ): KelirBuilder<TMode, "unsealed", Name, Ctx & NewCtx, Deps, Params> {
    this.checkVerb(".derive");
    this.registry.deriveFns.push(fn as (c: unknown) => unknown);
    return this as unknown as KelirBuilder<TMode, "unsealed", Name, Ctx & NewCtx, Deps, Params>;
  }

  // see: cans/03-sdk-architecture.md#TypeScript-SDK
  context<NewCtx>(
    this: KelirBuilder<"plugin", "unsealed", Name, Ctx, Deps, Params>,
    fn: (c: Ctx) => NewCtx,
  ): KelirBuilder<"plugin", "unsealed", Name, Ctx & NewCtx, Deps, Params> {
    this.checkVerb(".context");
    if (this.mode === "host") this.rejectPluginOnly(".context");
    this.registry.contextFns.push(fn as (c: unknown) => unknown);
    return this as unknown as KelirBuilder<"plugin", "unsealed", Name, Ctx & NewCtx, Deps, Params>;
  }

  // see: cans/artifacts/code/plugin-trait.rs#MountScope
  scope(
    this: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>,
    scope: MountScope,
  ): KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params> {
    this.checkVerb(".scope");
    this.registry.setScope(scope);
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  group(
    this: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>,
    fn: (builder: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>) => KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params> | void,
  ): KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params> {
    this.checkVerb(".group");
    const result = fn(this);
    if (result !== undefined && result !== this) throw new EConfig("group() must return the builder it receives");
    return this;
  }

  // see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
  guard(
    this: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>,
    enabled: boolean,
    fn: (builder: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>) => KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params> | void,
  ): KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params> {
    this.checkVerb(".guard");
    if (!enabled) return this;
    const result = fn(this);
    if (result !== undefined && result !== this) throw new EConfig("guard() must return the builder it receives");
    return this;
  }

  // see: cans/02-plugin-model.md#KelirPlugin-trait
  forkWith(
    this: KelirBuilder<"plugin", "unsealed", Name, Ctx, Deps, Params>,
    fn: () => AnyBuilder,
  ): KelirBuilder<"plugin", "unsealed", Name, Ctx, Deps, Params> {
    this.checkVerb(".forkWith");
    if (this.mode === "host") this.rejectPluginOnly(".forkWith");
    this.forkFn = fn;
    return this;
  }

  forkState(): AnyBuilder {
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

  private clone(): AnyBuilder {
    const copy = this.mode === "plugin"
      ? new KelirBuilder(this.name, this.storedOptions)
      : new KelirBuilder(this.kelirConfig);
    copy.registry = this.registry.clone();
    copy.mounted = [...this.mounted];
    copy.pluginRegistrations = [...this.pluginRegistrations];
    copy.mountRecords = [...this.mountRecords];
    copy.detachedFlag = this.detachedFlag;
    copy.terminalCfg = this.terminalCfg;
    copy.storageCfg = this.storageCfg;
    copy.wireCfg = this.wireCfg;
    copy.hostCfg = this.hostCfg;
    copy.checkpointCfg = this.checkpointCfg;
    copy.lockBrokerCfg = this.lockBrokerCfg;
    copy.paramsSchema = this.paramsSchema;
    copy.forkFn = this.forkFn;
    return copy;
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
  seal(): KelirBuilder<TMode, "sealed", Name, Ctx, Deps, Params> {
    if (!this.sealedFlag) {
      // see: cans/02-plugin-model.md#Nested-scopes
      const broker = this.registry.fsScopeConfig?.lockBroker;
      if (broker !== undefined) this.lockBrokerCfg = broker;
      this.validate();
      this.validateMounts();
      this.sealedFlag = true;
    }
    return this as unknown as KelirBuilder<TMode, "sealed", Name, Ctx, Deps, Params>;
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

  // see: cans/02-plugin-model.md#One-validation,-at-seal-and-boot
  async boot(this: KelirBuilder<"host", BuilderSealState, string, Ctx, Deps, Params>): Promise<Session & Ctx> {
    if (this.mode === "plugin") {
      throw new EConfig(
        `cannot boot plugin ${this.name}: mount it on a host with .use()`,
      );
    }
    if (!this.sealedFlag) this.seal();
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
        wireframeGrid: this.terminalCfg.wireframeGrid,
        sanitizer: this.terminalCfg.sanitizer,
      });
    }
    if (this.storageCfg !== undefined || this.lockBrokerCfg !== undefined) {
      nativeBuilder.setStorageConfig({
        idmapGuestUid: this.storageCfg?.idmap?.guestUid,
        idmapHostUid: this.storageCfg?.idmap?.hostUid,
        sharedCaches: this.storageCfg?.sharedCaches,
        lockBroker: this.lockBrokerCfg !== undefined,
        lockBrokerGlobs: this.lockBrokerCfg?.globs as string[] | undefined,
        lockBrokerNonBlocking: this.lockBrokerCfg?.nonBlocking,
        encapsulation: this.storageCfg?.encapsulation,
        daxWindow: this.storageCfg?.daxWindow,
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
        blockAbstractUnixSockets: this.wireCfg.blockAbstractUnixSockets,
      });
    }
    // TODO (L3) [bindings_ts]: forward host/checkpoint plane configs once the NAPI builder accepts them.; slop: host and checkpoint descriptors stay TypeScript-side
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
    const session = new Session(nativeSession, this.profileSpec);
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

export function kelir(config: KelirConfig = {}): KelirBuilder<"host", "unsealed"> {
  return new KelirBuilder(config);
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
export function createPlugin<Name extends string, Ctx = unknown, Opts = unknown, Deps extends string = never, Params = unknown>(
  name: Name, options?: CreatePluginOptions<Opts> | Ctx,
): KelirBuilder<"plugin", "unsealed", Name, Ctx, Deps, Params> {
  return new KelirBuilder<"plugin", "unsealed", Name, Ctx, Deps, Params>(name, options);
}
