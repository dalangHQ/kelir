// ILLUSTRATIVE, NON-STANDALONE — do not compile this file directly.
// It pins the TypeScript unified builder surface; the live shape is
// packages/ts/core/src/plugin.model.ts + builder.lifecycle.ts (Agent 4)
// and the normative contract is
// cans/02-plugin-model.md#Unified-builder (Agent 1).
//
// Target shape: ONE class KelirBuilder. new KelirBuilder(config) is the
// host; createPlugin(name, options?) returns a plugin-mode KelirBuilder.
// The 24 ring verbs + 8 closure scopes + context / requires / scope /
// forkWith / vsockService exist in BOTH modes. Mount is merge by value:
// host.use(plugin) appends the plugin's hooksForFusion after the host's
// own hooks. Validation happens once, in seal(), over the host +
// mounted plugins together; boot() seals implicitly when the builder is
// not sealed yet. There is no build(): a plugin-mode builder is mounted
// as-is, and a verb after seal() throws EConfig (the builder is sealed).
// fuse() is internal, not a chain verb and not a terminal; boot() is the
// single terminal. PluginContract / BuiltPlugin are deleted.
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
export type ValidateDeps<Ctx, P> = P extends KelirBuilder<any, any, infer Deps, any>
  ? [Deps] extends [never]
    ? unknown
    : string extends Deps
      ? unknown
      : [Deps] extends [keyof Ctx]
        ? unknown
        : { readonly __missing_dependency: Deps }
  : unknown;

// see: cans/02-plugin-model.md#Unified-builder
export interface TrapScope<Name extends string, Ctx, Deps extends string> {
  onTrap(
    handler: (event: TrapInfo, c: TrapContext & Ctx) => Promise<Verdict | null> | Verdict | null,
  ): TrapScope<Name, Ctx, Deps>;
  onProcessExit(
    handler: (event: ProcessExitEvent) => Promise<void> | void,
  ): TrapScope<Name, Ctx, Deps>;
  end(): KelirBuilder<Name, Ctx, Deps, unknown>;
}

// see: cans/02-plugin-model.md#Unified-builder
export interface FsScope<Name extends string, Ctx, Deps extends string> {
  onFsLookup(handler: (path: string) => Verdict): FsScope<Name, Ctx, Deps>;
  onFsRead(
    handler: (path: string, buffer: Uint8Array) => Uint8Array,
  ): FsScope<Name, Ctx, Deps>;
  onFsWrite(
    handler: (
      path: string,
      buffer: Uint8Array,
      pid: number,
      ctx: TrapContext & Ctx,
    ) => FilterAction,
  ): FsScope<Name, Ctx, Deps>;
  onFsLock(
    handler: (
      path: string,
      flockCmd: number,
      flockType: number,
      pid: number,
    ) => Verdict,
  ): FsScope<Name, Ctx, Deps>;
  end(): KelirBuilder<Name, Ctx, Deps, unknown>;
}

// see: cans/02-plugin-model.md#Unified-builder
export interface NetScope<Name extends string, Ctx, Deps extends string> {
  onNetOutbound(handler: (frame: Uint8Array) => FilterAction): NetScope<Name, Ctx, Deps>;
  onNetInbound(handler: (frame: Uint8Array) => FilterAction): NetScope<Name, Ctx, Deps>;
  end(): KelirBuilder<Name, Ctx, Deps, unknown>;
}

// see: cans/02-plugin-model.md#Unified-builder
export interface ConsoleScope<Name extends string, Ctx, Deps extends string> {
  onConsoleInput(
    handler: (port: number, bytes: Uint8Array) => Promise<Uint8Array> | Uint8Array,
  ): ConsoleScope<Name, Ctx, Deps>;
  onConsoleOutput(
    handler: (port: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array,
  ): ConsoleScope<Name, Ctx, Deps>;
  end(): KelirBuilder<Name, Ctx, Deps, unknown>;
}

// see: cans/02-plugin-model.md#Unified-builder
export interface ExecScope<Name extends string, Ctx, Deps extends string> {
  onExecStdout(
    handler: (
      pid: number,
      execId: number,
      chunk: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): ExecScope<Name, Ctx, Deps>;
  onExecStderr(
    handler: (
      pid: number,
      execId: number,
      chunk: Uint8Array,
    ) => Promise<Uint8Array> | Uint8Array,
  ): ExecScope<Name, Ctx, Deps>;
  end(): KelirBuilder<Name, Ctx, Deps, unknown>;
}

// see: cans/02-plugin-model.md#Unified-builder
export interface HostScope<Name extends string, Ctx, Deps extends string> {
  onHostLsmExec(
    handler: (path: string, pid: number) => Verdict,
  ): HostScope<Name, Ctx, Deps>;
  onHostPressure(handler: (level: number) => Verdict): HostScope<Name, Ctx, Deps>;
  onHostKeyringAccess(handler: (key: string) => Verdict): HostScope<Name, Ctx, Deps>;
  onHostPreempt(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): HostScope<Name, Ctx, Deps>;
  end(): KelirBuilder<Name, Ctx, Deps, unknown>;
}

// see: cans/02-plugin-model.md#Unified-builder
export interface LifecycleScope<Name extends string, Ctx, Deps extends string> {
  onVmInit(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): LifecycleScope<Name, Ctx, Deps>;
  onVmReady(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): LifecycleScope<Name, Ctx, Deps>;
  onVmShutdown(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): LifecycleScope<Name, Ctx, Deps>;
  onPreCommit(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): LifecycleScope<Name, Ctx, Deps>;
  onPostCommit(
    handler: (ctx: LifecycleContext, committed: boolean) => Promise<void> | void,
  ): LifecycleScope<Name, Ctx, Deps>;
  end(): KelirBuilder<Name, Ctx, Deps, unknown>;
}

// see: cans/02-plugin-model.md#Unified-builder
export interface TelemetryScope<Name extends string, Ctx, Deps extends string> {
  onMemoryPressure(
    handler: (used: number, total: number) => Promise<void> | void,
  ): TelemetryScope<Name, Ctx, Deps>;
  onHeartbeat(
    handler: (event: HeartbeatEvent) => Promise<void> | void,
  ): TelemetryScope<Name, Ctx, Deps>;
  onWatchdog(
    handler: (event: WatchdogEvent) => Promise<WatchdogVerdict> | WatchdogVerdict,
  ): TelemetryScope<Name, Ctx, Deps>;
  end(): KelirBuilder<Name, Ctx, Deps, unknown>;
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
  Name extends string = string,
  Ctx = unknown,
  Deps extends string = never,
  Params = unknown,
> {
  constructor(config?: KelirConfig);

  requires<Req extends string>(): KelirBuilder<Name, Ctx, Deps | Req, Params>;
  requires<P extends KelirBuilder<string, unknown, string, unknown>>(
    plugin: P,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  derive<NewCtx>(fn: (c: Ctx) => NewCtx): KelirBuilder<Name, Ctx & NewCtx, Deps, Params>;
  context<NewCtx>(fn: (c: Ctx) => NewCtx): KelirBuilder<Name, Ctx & NewCtx, Deps, Params>;
  scope(scope: MountScope): KelirBuilder<Name, Ctx, Deps, Params>;
  forkWith(
    fn: () => KelirBuilder<Name, Ctx, Deps, Params>,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  vsockService(
    port: number,
    service: string,
    schema: unknown,
    handler: (request: unknown) => Promise<unknown> | unknown,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  onTrap(
    handler: (event: TrapInfo, c: TrapContext & Ctx) => Promise<Verdict | null> | Verdict | null,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  onProcessExit(
    handler: (event: ProcessExitEvent) => Promise<void> | void,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  onFsLookup(handler: (path: string) => Verdict): KelirBuilder<Name, Ctx, Deps, Params>;
  onFsRead(
    handler: (path: string, buffer: Uint8Array) => Uint8Array,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  onFsWrite(
    handler: (
      path: string,
      buffer: Uint8Array,
      pid: number,
      ctx: TrapContext & Ctx,
    ) => FilterAction,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  onFsLock(
    handler: (path: string, flockCmd: number, flockType: number, pid: number) => Verdict,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  onNetOutbound(
    handler: (frame: Uint8Array) => FilterAction,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  onNetInbound(
    handler: (frame: Uint8Array) => FilterAction,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  onConsoleInput(
    handler: (port: number, bytes: Uint8Array) => Promise<Uint8Array> | Uint8Array,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  onConsoleOutput(
    handler: (port: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  onExecStdout(
    handler: (pid: number, execId: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  onExecStderr(
    handler: (pid: number, execId: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  onHostLsmExec(
    handler: (path: string, pid: number) => Verdict,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  onHostPressure(
    handler: (level: number) => Verdict,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  onHostKeyringAccess(
    handler: (key: string) => Verdict,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  onVmInit(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  onVmReady(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  onVmShutdown(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  onPreCommit(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  onPostCommit(
    handler: (ctx: LifecycleContext, committed: boolean) => Promise<void> | void,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  onHostPreempt(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  onMemoryPressure(
    handler: (used: number, total: number) => Promise<void> | void,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  onHeartbeat(
    handler: (event: HeartbeatEvent) => Promise<void> | void,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  onWatchdog(
    handler: (event: WatchdogEvent) => Promise<WatchdogVerdict> | WatchdogVerdict,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  trap(fn: (scope: TrapScope<Name, Ctx, Deps>) => TrapScope<Name, Ctx, Deps> | void): KelirBuilder<Name, Ctx, Deps, Params>;
  trap(
    filter: string | TrapFilter,
    handler: TrapHandler<Ctx, Params> | TrapHandlers<Ctx, Params>,
  ): KelirBuilder<Name, Ctx, Deps, Params>;
  fs(fn: (scope: FsScope<Name, Ctx, Deps>) => FsScope<Name, Ctx, Deps> | void): KelirBuilder<Name, Ctx, Deps, Params>;
  net(fn: (scope: NetScope<Name, Ctx, Deps>) => NetScope<Name, Ctx, Deps> | void): KelirBuilder<Name, Ctx, Deps, Params>;
  console(fn: (scope: ConsoleScope<Name, Ctx, Deps>) => ConsoleScope<Name, Ctx, Deps> | void): KelirBuilder<Name, Ctx, Deps, Params>;
  exec(fn: (scope: ExecScope<Name, Ctx, Deps>) => ExecScope<Name, Ctx, Deps> | void): KelirBuilder<Name, Ctx, Deps, Params>;
  host(fn: (scope: HostScope<Name, Ctx, Deps>) => HostScope<Name, Ctx, Deps> | void): KelirBuilder<Name, Ctx, Deps, Params>;
  lifecycle(fn: (scope: LifecycleScope<Name, Ctx, Deps>) => LifecycleScope<Name, Ctx, Deps> | void): KelirBuilder<Name, Ctx, Deps, Params>;
  telemetry(fn: (scope: TelemetryScope<Name, Ctx, Deps>) => TelemetryScope<Name, Ctx, Deps> | void): KelirBuilder<Name, Ctx, Deps, Params>;
  group(fn: (builder: this) => this | void): this;
  guard(enabled: boolean, fn: (builder: this) => this): this;
  forkState(): KelirBuilder<Name, Ctx, Deps, Params>;

  hooksForFusion(): readonly HookDescriptor[];

  use<P extends KelirBuilder<string, unknown, string, unknown> | KelirPlugin>(
    plugin: P & ValidateDeps<Ctx, P>,
  ): KelirBuilder<Name, Ctx & PluginContextOf<P>, Deps, Params>;

  plugin<P extends KelirBuilder<string, unknown, string, unknown> | KelirPlugin>(
    plugin: P & ValidateDeps<Ctx, P>,
  ): KelirBuilder<Name, Ctx & PluginContextOf<P>, Deps, Params>;

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
  seal(): this;
  boot(): Promise<Session & Ctx>;
}

// see: cans/02-plugin-model.md#Unified-builder
// The plugin-mode factory: createPlugin returns a KelirBuilder in plugin
// mode. There is no build() to call after it.
export declare function createPlugin<Name extends string>(
  name: Name,
  options?: CreatePluginOptions,
): KelirBuilder<Name, unknown, never, unknown>;

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
