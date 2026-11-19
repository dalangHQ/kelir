// ILLUSTRATIVE, NON-STANDALONE — do not compile this file directly.
// It pins the TypeScript unified builder surface; the live shape is
// packages/ts/core/src/plugin.model.ts + builder.lifecycle.ts (Agent 4)
// and the normative contract is
// cans/02-plugin-model.md#Unified-builder (Agent 1).
//
// Target shape: ONE class KelirBuilder<TMode, TSealed, Name, Ctx, Deps,
// Params>. new KelirBuilder(config) is the host; createPlugin(name,
// options?) returns a plugin-mode KelirBuilder. The 8 closure scopes +
// context / requires / scope / forkWith exist in BOTH modes; ring verbs
// exist ONLY inside their scope closures (the flat builder-surface
// on* verbs, top-level .on(), .onTrapDeny(), .registerCustomAction(),
// .registerStreamKind(), .vsockService(), .lockBroker() and the
// direct-model .params(schema) are deleted; a flat chain survives only
// as a group() fold). TMode ("host"|"plugin") and TSealed
// ("unsealed"|"sealed") ride private phantom markers so this:-parameter
// gates reject host verbs on plugin builders, plugin-only verbs on
// hosts, and any config verb after .seal() at compile time; the runtime
// EConfig traps stay for JS callers. Mount is merge by value:
// host.use(plugin) appends the plugin's hooks after the host's own
// hooks. Validation happens once, in seal(), over the host + mounted
// plugins together; boot() requires host mode and seals implicitly when
// the builder is not sealed yet. There is no build(): a plugin-mode
// builder is mounted as-is. fuse() is internal, not a chain verb and not
// a terminal; boot() is the single terminal.
//
// Owned by: cans/02-plugin-model.md#Unified-builder
// see: cans/02-plugin-model.md#Unified-builder
// see: cans/03-sdk-architecture.md#Unified-builder-surface
// see: cans/artifacts/examples/fluent-builder-chaos.ts#buildChaosPlugin

import type { LifecycleContext, TrapContext } from "./context.lifecycle.types.js";
import type { Infer, Schema } from "./schema.config.js";
import type {
  FilterAction,
  HeartbeatEvent,
  KelirPlugin,
  MountScope,
  PluginContextOf,
  ProcessExitEvent,
  TrapInfo,
  Verdict,
  WatchdogEvent,
  WatchdogVerdict,
} from "./plugin.model.js";
import type { CreatePluginOptions } from "./plugin.model.js";
import type {
  KelirConfig,
  StorageConfig,
  TerminalConfig,
  TrapHandler,
  TrapHandlers,
  WireConfig,
} from "./builder.lifecycle.js";
import type { TrapFilter } from "./trap.protocol.js";
import type { FusedTrampoline, HookDescriptor } from "./fusion.plugin.js";
import type { Session } from "./session.lifecycle.js";
import type { TrapDenyEvent } from "./types.core.js";
import type { LifecycleEvent } from "./verdict.protocol.js";

// see: cans/02-plugin-model.md#Unified-builder
export type ValidateDeps<Ctx, P> = P extends KelirBuilder<any, any, any, any, infer Deps, any>
  ? [Deps] extends [never]
    ? unknown
    : string extends Deps
      ? unknown
      : [Deps] extends [keyof Ctx]
        ? unknown
        : { readonly __missing_dependency: Deps }
  : unknown;

// see: cans/02-plugin-model.md#Unified-builder
export interface TrapScope<TMode extends "host" | "plugin", TSealed extends "unsealed" | "sealed", Name extends string, Ctx, Deps extends string> {
  onTrap(
    handler: (event: TrapInfo, c: TrapContext & Ctx) => Promise<Verdict | null> | Verdict | null,
  ): TrapScope<TMode, TSealed, Name, Ctx, Deps>;
  onProcessExit(
    handler: (event: ProcessExitEvent) => Promise<void> | void,
  ): TrapScope<TMode, TSealed, Name, Ctx, Deps>;
  onSignal(
    handler: (sender: number, target: number, sig: number) => Promise<Verdict> | Verdict,
  ): TrapScope<TMode, TSealed, Name, Ctx, Deps>;
  onFork(
    handler: (parent: number, child: number) => Promise<Verdict> | Verdict,
  ): TrapScope<TMode, TSealed, Name, Ctx, Deps>;
  end(): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, unknown>;
}

// see: cans/02-plugin-model.md#Unified-builder
export interface FsScope<TMode extends "host" | "plugin", TSealed extends "unsealed" | "sealed", Name extends string, Ctx, Deps extends string> {
  onFsLookup(handler: (path: string) => Verdict): FsScope<TMode, TSealed, Name, Ctx, Deps>;
  onFsRead(
    handler: (path: string, buffer: Uint8Array) => Uint8Array,
  ): FsScope<TMode, TSealed, Name, Ctx, Deps>;
  onFsWrite(
    handler: (
      path: string,
      buffer: Uint8Array,
      pid: number,
      ctx: TrapContext & Ctx,
    ) => FilterAction,
  ): FsScope<TMode, TSealed, Name, Ctx, Deps>;
  onFsLock(
    handler: (
      path: string,
      flockCmd: number,
      flockType: number,
      pid: number,
    ) => Verdict,
  ): FsScope<TMode, TSealed, Name, Ctx, Deps>;
  onFsUnlink(handler: (path: string) => Verdict): FsScope<TMode, TSealed, Name, Ctx, Deps>;
  onFsRename(handler: (src: string, dst: string) => Verdict): FsScope<TMode, TSealed, Name, Ctx, Deps>;
  onFsSetattr(
    handler: (path: string, mode: number, size: number) => Verdict,
  ): FsScope<TMode, TSealed, Name, Ctx, Deps>;
  onFsSymlink(handler: (target: string, link: string) => Verdict): FsScope<TMode, TSealed, Name, Ctx, Deps>;
  onFsMkdir(handler: (path: string, mode: number) => Verdict): FsScope<TMode, TSealed, Name, Ctx, Deps>;
  onFsReaddir(
    handler: (path: string, entries: unknown[]) => FilterAction,
  ): FsScope<TMode, TSealed, Name, Ctx, Deps>;
  onFsReadlink(handler: (path: string) => Verdict): FsScope<TMode, TSealed, Name, Ctx, Deps>;
  end(): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, unknown>;
}

// see: cans/02-plugin-model.md#Unified-builder
export interface NetScope<TMode extends "host" | "plugin", TSealed extends "unsealed" | "sealed", Name extends string, Ctx, Deps extends string> {
  onNetOutbound(handler: (frame: Uint8Array) => FilterAction): NetScope<TMode, TSealed, Name, Ctx, Deps>;
  onNetInbound(handler: (frame: Uint8Array) => FilterAction): NetScope<TMode, TSealed, Name, Ctx, Deps>;
  onSocketConnect(
    handler: (pid: number, proto: number, ip: [number, number, number, number], port: number) => Verdict,
  ): NetScope<TMode, TSealed, Name, Ctx, Deps>;
  onDnsQuery(
    handler: (pid: number, domain: string, qtype: number) => unknown,
  ): NetScope<TMode, TSealed, Name, Ctx, Deps>;
  onSocketBind(
    handler: (pid: number, proto: number, ip: [number, number, number, number], port: number) => Verdict,
  ): NetScope<TMode, TSealed, Name, Ctx, Deps>;
  end(): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, unknown>;
}

// see: cans/02-plugin-model.md#Unified-builder
export interface ConsoleScope<TMode extends "host" | "plugin", TSealed extends "unsealed" | "sealed", Name extends string, Ctx, Deps extends string> {
  onConsoleInput(
    handler: (port: number, bytes: Uint8Array) => Promise<Uint8Array> | Uint8Array,
  ): ConsoleScope<TMode, TSealed, Name, Ctx, Deps>;
  onConsoleOutput(
    handler: (port: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array,
  ): ConsoleScope<TMode, TSealed, Name, Ctx, Deps>;
  end(): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, unknown>;
}

// see: cans/02-plugin-model.md#Unified-builder
export interface ExecScope<TMode extends "host" | "plugin", TSealed extends "unsealed" | "sealed", Name extends string, Ctx, Deps extends string> {
  onExecStdout(
    handler: (
      pid: number,
      execId: number,
      chunk: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): ExecScope<TMode, TSealed, Name, Ctx, Deps>;
  onExecStderr(
    handler: (
      pid: number,
      execId: number,
      chunk: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): ExecScope<TMode, TSealed, Name, Ctx, Deps>;
  onExecStdin(
    handler: (
      pid: number,
      execId: number,
      chunk: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): ExecScope<TMode, TSealed, Name, Ctx, Deps>;
  end(): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, unknown>;
}

// see: cans/02-plugin-model.md#Unified-builder
export interface HostScope<TMode extends "host" | "plugin", TSealed extends "unsealed" | "sealed", Name extends string, Ctx, Deps extends string> {
  onHostLsmExec(
    handler: (path: string, pid: number) => Verdict,
  ): HostScope<TMode, TSealed, Name, Ctx, Deps>;
  onHostPressure(handler: (level: number) => Verdict): HostScope<TMode, TSealed, Name, Ctx, Deps>;
  onHostKeyringAccess(handler: (key: string) => Verdict): HostScope<TMode, TSealed, Name, Ctx, Deps>;
  onHostPreempt(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): HostScope<TMode, TSealed, Name, Ctx, Deps>;
  onHostContainmentBreach(
    handler: (pid: number, path: string, kind: string) => void,
  ): HostScope<TMode, TSealed, Name, Ctx, Deps>;
  end(): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, unknown>;
}

// see: cans/02-plugin-model.md#Unified-builder
export interface LifecycleScope<TMode extends "host" | "plugin", TSealed extends "unsealed" | "sealed", Name extends string, Ctx, Deps extends string> {
  onVmInit(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): LifecycleScope<TMode, TSealed, Name, Ctx, Deps>;
  onVmReady(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): LifecycleScope<TMode, TSealed, Name, Ctx, Deps>;
  onVmShutdown(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): LifecycleScope<TMode, TSealed, Name, Ctx, Deps>;
  onPreCommit(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): LifecycleScope<TMode, TSealed, Name, Ctx, Deps>;
  onPostCommit(
    handler: (ctx: LifecycleContext, committed: boolean) => Promise<void> | void,
  ): LifecycleScope<TMode, TSealed, Name, Ctx, Deps>;
  end(): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, unknown>;
}

// see: cans/02-plugin-model.md#Unified-builder
export interface TelemetryScope<TMode extends "host" | "plugin", TSealed extends "unsealed" | "sealed", Name extends string, Ctx, Deps extends string> {
  onMemoryPressure(
    handler: (used: number, total: number) => Promise<void> | void,
  ): TelemetryScope<TMode, TSealed, Name, Ctx, Deps>;
  onHeartbeat(
    handler: (event: HeartbeatEvent) => Promise<void> | void,
  ): TelemetryScope<TMode, TSealed, Name, Ctx, Deps>;
  onWatchdog(
    handler: (event: WatchdogEvent) => Promise<WatchdogVerdict> | WatchdogVerdict,
  ): TelemetryScope<TMode, TSealed, Name, Ctx, Deps>;
  end(): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, unknown>;
}

// see: cans/02-plugin-model.md#Unified-builder
// Target surface. Structural verbs: context, derive, requires, scope,
// forkWith, vsockService, group, guard. Ring verbs: the 24 on* methods. Scope entry
// points take a closure receiving the scope builder and returning it or
// void; every scope has .end() returning the builder. Host-only verbs (terminal,
// storage, wire, params, detached, seal, on, onTrapDeny,
// registerCustomAction, registerStreamKind, lockBroker,
// use, plugin) throw EConfig at the call site on a
// plugin-mode builder. trap(filter, handler) and trap(closure) are
// both modes under one overloaded name. The canonical chaos plugin is
// not built here: it lives in
// cans/artifacts/examples/fluent-builder-chaos.ts, the one shape shared
// by all three SDKs.
export declare class KelirBuilder<
  TMode extends "host" | "plugin" = "host" | "plugin",
  TSealed extends "unsealed" | "sealed" = "unsealed" | "sealed",
  Name extends string = string,
  Ctx = unknown,
  Deps extends string = never,
  Params = unknown,
> {
  private readonly __mode: TMode;
  private readonly __seal: TSealed;
  constructor(config?: KelirConfig);

  requires<Req extends string>(): KelirBuilder<TMode, TSealed, Name, Ctx, Deps | Req, Params>;
  requires<P extends KelirBuilder<string, unknown, string, unknown>>(
    plugin: P,
  ): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, Params>;
  derive<NewCtx>(fn: (c: Ctx) => NewCtx): KelirBuilder<TMode, TSealed, Name, Ctx & NewCtx, Deps, Params>;
  context<NewCtx>(fn: (c: Ctx) => NewCtx): KelirBuilder<TMode, TSealed, Name, Ctx & NewCtx, Deps, Params>;
  scope(scope: MountScope): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, Params>;
  forkWith(
    fn: () => KelirBuilder<TMode, TSealed, Name, Ctx, Deps, Params>,
  ): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, Params>;
  trap(fn: (scope: TrapScope<TMode, TSealed, Name, Ctx, Deps>) => TrapScope<Name, Ctx, Deps> | void): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, Params>;
  trap(
    filter: string | TrapFilter,
    handler: TrapHandler<Ctx, Params> | TrapHandlers<Ctx, Params>,
  ): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, Params>;
  fs(fn: (scope: FsScope<TMode, TSealed, Name, Ctx, Deps>) => FsScope<Name, Ctx, Deps> | void): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, Params>;
  net(fn: (scope: NetScope<TMode, TSealed, Name, Ctx, Deps>) => NetScope<Name, Ctx, Deps> | void): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, Params>;
  console(fn: (scope: ConsoleScope<TMode, TSealed, Name, Ctx, Deps>) => ConsoleScope<Name, Ctx, Deps> | void): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, Params>;
  exec(fn: (scope: ExecScope<TMode, TSealed, Name, Ctx, Deps>) => ExecScope<Name, Ctx, Deps> | void): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, Params>;
  host(fn: (scope: HostScope<TMode, TSealed, Name, Ctx, Deps>) => HostScope<Name, Ctx, Deps> | void): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, Params>;
  lifecycle(fn: (scope: LifecycleScope<TMode, TSealed, Name, Ctx, Deps>) => LifecycleScope<Name, Ctx, Deps> | void): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, Params>;
  telemetry(fn: (scope: TelemetryScope<TMode, TSealed, Name, Ctx, Deps>) => TelemetryScope<Name, Ctx, Deps> | void): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, Params>;
  group(
    this: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>,
    fn: (builder: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>) => KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params> | void,
  ): KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>;
  guard(
    this: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>,
    enabled: boolean,
    fn: (builder: KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>) => KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params> | void,
  ): KelirBuilder<TMode, "unsealed", Name, Ctx, Deps, Params>;
  forkState(): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, Params>;

  hooksForFusion(): readonly HookDescriptor[];

  use<P extends KelirBuilder<string, unknown, string, unknown> | KelirPlugin>(
    plugin: P & ValidateDeps<Ctx, P>,
  ): KelirBuilder<TMode, TSealed, Name, Ctx & PluginContextOf<P>, Deps, Params>;

  plugin<P extends KelirBuilder<string, unknown, string, unknown> | KelirPlugin>(
    plugin: P & ValidateDeps<Ctx, P>,
  ): KelirBuilder<TMode, TSealed, Name, Ctx & PluginContextOf<P>, Deps, Params>;

  terminal(config: TerminalConfig): this;
  storage<C = Ctx>(config: StorageConfig<C>): KelirBuilder<Name, Ctx & C, Deps, Params>;
  wire(config: WireConfig): this;
  params<S extends Schema<any>>(schema: S): KelirBuilder<Name, Ctx, Deps, Infer<S>>;
  detached(detached: boolean): this;
  lockBroker(config: { readonly globs: readonly string[]; readonly nonBlocking?: boolean }): this;
  isDetached(): boolean;
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
  ): this;
  onTrapDeny(
    handler: (
      event: TrapDenyEvent,
      session: Session & Ctx,
    ) => Promise<void> | void,
  ): this;
  registerCustomAction<P>(
    name: string,
    schema: Schema<P>,
    handler: (payload: P) => Promise<void> | void,
  ): this;
  registerStreamKind<Req, Resp = unknown>(
    kind: number,
    schema: Schema<Req>,
    handler: (request: Req) => Promise<Resp> | Resp,
  ): this;
  // see: cans/02-plugin-model.md#Unified-builder
  // seal() is the one validation; fuse() is internal, not a chain verb.
  seal(): KelirBuilder<TMode, "sealed", Name, Ctx, Deps, Params>;
  boot(
    this: KelirBuilder<"host", TSealed, Name, Ctx, Deps, Params>,
  ): Promise<Session & Ctx>;
}

// see: cans/02-plugin-model.md#Unified-builder
// The plugin-mode factory: createPlugin returns a KelirBuilder in plugin
// mode. There is no build() to call after it.
export declare function createPlugin<Name extends string>(
  name: Name,
  options?: CreatePluginOptions,
): KelirBuilder<"plugin", "unsealed", Name, unknown, never, unknown>;

// see: cans/02-plugin-model.md#Unified-builder
// Deleted shapes: PluginContract, BuiltPlugin, build(). Nothing is
// documented as an alternative: the old shapes are gone, not deprecated.
// Mount: use() merges the plugin's hooksForFusion after the host's own
// hooks (host hooks first, then plugins in mount order; within a plugin,
// registration order). plugin(builder) is the same verb and delegates to
// use(); a string argument to plugin() throws a clear EConfig. fuse() sorts each ring by ordinal and throws on
// a duplicate ordinal, so use() re-ordinals the mounted plugin's hooks
// into the host's sequence as it merges them; the plugin builder's own
// ordinals are untouched. Mounting the same plugin twice is recorded at
// use() and throws EConfig at seal() with `plugin already mounted:
// <name>`. A missing prerequisite throws EConfig at seal() with
// `missing prerequisite: <dep> required by <plugin>`; the type-level
// ValidateDeps above rejects it at compile time when the dep is
// statically known. seal() is the one validation: the vsock triple
// (port 1..=65535, non-empty service) applies to host-registered and
// plugin services alike (vsockService keeps its live u32 call-site
// check as the type-range guard) and the scope triple (non-empty paths).
// Host-only verbs on a plugin-mode builder throw EConfig at the call site.
// An empty builder (no handlers) still seals. fuse() is internal:
// fuse().describe() is the one descriptor path, called inside seal()
// and boot(), never by the caller. Examples call .seal() before .boot();
// boot() seals implicitly only when the caller omits it. trap() has two overloads on the one
// class: the scope entry point taking a closure, and the filter form
// taking (filter, handler); they resolve by argument type, in both
// modes. forkState() is a factory, not a chain verb: it returns a fresh
// plugin-mode builder, and it throws EConfig on a host-mode builder;
// hosts fork via Session.fork(). group(fn) runs fn on the same builder;
// guard(enabled, fn) runs fn only when enabled is true. The vsock spec is (port, service, schema) in TS: the
// schema field is plugin-level in TS/Python and absent in Rust.
// lockBroker(config) is a host-only verb; only lockBroker: true reaches
// the native addon, globs/nonBlocking forwarding is L3.
