// ILLUSTRATIVE, NON-STANDALONE — do not compile this file directly.
// It pins the TypeScript fluent plugin-creation builder surface; the live
// shape is packages/ts/core/src/plugin.model.ts (Agent 4) and the normative
// contract is cans/02-plugin-model.md#Fluent-plugin-creation-builder.
//
// Target shape: the 30-verb closed set (6 structural + 24 ring), the 8
// closure-scoped entry points, and the validating build. Verbs marked LIVE
// already exist on PluginContract; verbs marked A4 are the DX-4 contract —
// they do not exist on the live class yet. The old artifact claimed
// .onFsLock / .onNetInbound / .onVmInit as live; that claim was false and
// is corrected here. Flat verbs stay; a nested chain and the equivalent
// flat chain produce identical descriptors (same rings, same ordinals).
// The canonical chaos plugin is not built here: it lives in
// cans/artifacts/examples/fluent-builder-chaos.ts, the one shape shared by
// all three SDKs.
//
// Owned by: cans/02-plugin-model.md#Fluent-plugin-creation-builder
// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
// see: cans/03-sdk-architecture.md#Fluent-plugin-surface
// see: cans/artifacts/examples/fluent-builder-chaos.ts#buildChaosPlugin

import type { LifecycleContext, TrapContext } from "./context.lifecycle.types.js";
import type {
  BuiltPlugin,
  FilterAction,
  HeartbeatEvent,
  MountScope,
  PluginContract,
  ProcessExitEvent,
  TrapInfo,
  Verdict,
  WatchdogEvent,
  WatchdogVerdict,
} from "./plugin.model.js";

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export type PluginBuilder<Ctx = unknown, Deps extends string = never> =
  PluginContract<string, Ctx, Deps>;

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export interface TrapScope<Name extends string, Ctx, Deps extends string> {
  onTrap(
    handler: (event: TrapInfo, c: TrapContext & Ctx) => Promise<Verdict | null> | Verdict | null,
  ): TrapScope<Name, Ctx, Deps>;
  onProcessExit(
    handler: (event: ProcessExitEvent) => Promise<void> | void,
  ): TrapScope<Name, Ctx, Deps>;
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
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
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export interface NetScope<Name extends string, Ctx, Deps extends string> {
  onNetOutbound(handler: (frame: Uint8Array) => FilterAction): NetScope<Name, Ctx, Deps>;
  onNetInbound(handler: (frame: Uint8Array) => FilterAction): NetScope<Name, Ctx, Deps>;
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export interface ConsoleScope<Name extends string, Ctx, Deps extends string> {
  onConsoleInput(
    handler: (port: number, bytes: Uint8Array) => Promise<Uint8Array> | Uint8Array,
  ): ConsoleScope<Name, Ctx, Deps>;
  onConsoleOutput(
    handler: (port: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array,
  ): ConsoleScope<Name, Ctx, Deps>;
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
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
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export interface HostScope<Name extends string, Ctx, Deps extends string> {
  onHostLsmExec(
    handler: (path: string, pid: number) => Verdict,
  ): HostScope<Name, Ctx, Deps>;
  onHostPressure(handler: (level: number) => Verdict): HostScope<Name, Ctx, Deps>;
  onHostKeyringAccess(handler: (key: string) => Verdict): HostScope<Name, Ctx, Deps>;
  onHostPreempt(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): HostScope<Name, Ctx, Deps>;
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
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
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
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
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
// Target surface. Structural verbs: context, requires, scope, forkWith,
// vsockService, build. Ring verbs: the 24 on* methods. LIVE marks the 12
// ring verbs already on PluginContract; A4 marks the 12 DX-4 additions.
// Scope entry points take a closure receiving the scope builder and
// returning it; there is no .end() / .done() verb. .build() throws EConfig
// when a registered verb resolves to no known ring, when a vsock spec is
// invalid (port outside 1..=65535, empty service), or when the scope value
// is malformed; an empty plugin (no handlers) still builds. Any verb after
// .build() throws; .use() rejects an unbuilt contract at runtime.
export interface PluginContractSurface<
  Name extends string = string,
  Ctx = unknown,
  Deps extends string = never,
> {
  requires<Req extends string | PluginContract | BuiltPlugin>(): PluginContractSurface<Name, Ctx, Deps>;
  derive<NewCtx>(fn: (c: Ctx) => NewCtx): PluginContractSurface<Name, Ctx & NewCtx, Deps>;
  context<NewCtx>(fn: (c: Ctx) => NewCtx): PluginContractSurface<Name, Ctx & NewCtx, Deps>;
  scope(scope: MountScope): PluginContractSurface<Name, Ctx, Deps>;
  forkWith(fn: () => PluginContract<Name, Ctx, Deps>): PluginContractSurface<Name, Ctx, Deps>;
  vsockService(
    port: number,
    service: string,
    schema: unknown,
    handler: (request: unknown) => Promise<unknown> | unknown,
  ): PluginContractSurface<Name, Ctx, Deps>;
  // LIVE
  onTrap(
    handler: (event: TrapInfo, c: TrapContext & Ctx) => Promise<Verdict | null> | Verdict | null,
  ): PluginContractSurface<Name, Ctx, Deps>;
  // LIVE
  onProcessExit(
    handler: (event: ProcessExitEvent) => Promise<void> | void,
  ): PluginContractSurface<Name, Ctx, Deps>;
  // LIVE
  onFsLookup(handler: (path: string) => Verdict): PluginContractSurface<Name, Ctx, Deps>;
  // LIVE
  onFsRead(
    handler: (path: string, buffer: Uint8Array) => Uint8Array,
  ): PluginContractSurface<Name, Ctx, Deps>;
  // LIVE
  onFsWrite(
    handler: (
      path: string,
      buffer: Uint8Array,
      pid: number,
      ctx: TrapContext & Ctx,
    ) => FilterAction,
  ): PluginContractSurface<Name, Ctx, Deps>;
  // A4
  onFsLock(
    handler: (path: string, flockCmd: number, flockType: number, pid: number) => Verdict,
  ): PluginContractSurface<Name, Ctx, Deps>;
  // LIVE
  onNetOutbound(
    handler: (frame: Uint8Array) => FilterAction,
  ): PluginContractSurface<Name, Ctx, Deps>;
  // A4
  onNetInbound(
    handler: (frame: Uint8Array) => FilterAction,
  ): PluginContractSurface<Name, Ctx, Deps>;
  // LIVE
  onConsoleInput(
    handler: (port: number, bytes: Uint8Array) => Promise<Uint8Array> | Uint8Array,
  ): PluginContractSurface<Name, Ctx, Deps>;
  // LIVE
  onConsoleOutput(
    handler: (port: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array,
  ): PluginContractSurface<Name, Ctx, Deps>;
  // LIVE
  onExecStdout(
    handler: (pid: number, execId: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array,
  ): PluginContractSurface<Name, Ctx, Deps>;
  // LIVE
  onExecStderr(
    handler: (pid: number, execId: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array,
  ): PluginContractSurface<Name, Ctx, Deps>;
  // A4
  onHostLsmExec(
    handler: (path: string, pid: number) => Verdict,
  ): PluginContractSurface<Name, Ctx, Deps>;
  // A4
  onHostPressure(
    handler: (level: number) => Verdict,
  ): PluginContractSurface<Name, Ctx, Deps>;
  // A4
  onHostKeyringAccess(
    handler: (key: string) => Verdict,
  ): PluginContractSurface<Name, Ctx, Deps>;
  // A4
  onVmInit(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): PluginContractSurface<Name, Ctx, Deps>;
  // A4
  onVmReady(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): PluginContractSurface<Name, Ctx, Deps>;
  // A4
  onVmShutdown(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): PluginContractSurface<Name, Ctx, Deps>;
  // A4
  onPreCommit(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): PluginContractSurface<Name, Ctx, Deps>;
  // A4
  onPostCommit(
    handler: (ctx: LifecycleContext, committed: boolean) => Promise<void> | void,
  ): PluginContractSurface<Name, Ctx, Deps>;
  // A4
  onHostPreempt(
    handler: (ctx: LifecycleContext) => Promise<void> | void,
  ): PluginContractSurface<Name, Ctx, Deps>;
  // A4
  onMemoryPressure(
    handler: (used: number, total: number) => Promise<void> | void,
  ): PluginContractSurface<Name, Ctx, Deps>;
  // LIVE
  onHeartbeat(
    handler: (event: HeartbeatEvent) => Promise<void> | void,
  ): PluginContractSurface<Name, Ctx, Deps>;
  // LIVE
  onWatchdog(
    handler: (event: WatchdogEvent) => Promise<WatchdogVerdict> | WatchdogVerdict,
  ): PluginContractSurface<Name, Ctx, Deps>;
  trap(fn: (scope: TrapScope<Name, Ctx, Deps>) => TrapScope<Name, Ctx, Deps>): PluginContractSurface<Name, Ctx, Deps>;
  fs(fn: (scope: FsScope<Name, Ctx, Deps>) => FsScope<Name, Ctx, Deps>): PluginContractSurface<Name, Ctx, Deps>;
  net(fn: (scope: NetScope<Name, Ctx, Deps>) => NetScope<Name, Ctx, Deps>): PluginContractSurface<Name, Ctx, Deps>;
  console(fn: (scope: ConsoleScope<Name, Ctx, Deps>) => ConsoleScope<Name, Ctx, Deps>): PluginContractSurface<Name, Ctx, Deps>;
  exec(fn: (scope: ExecScope<Name, Ctx, Deps>) => ExecScope<Name, Ctx, Deps>): PluginContractSurface<Name, Ctx, Deps>;
  host(fn: (scope: HostScope<Name, Ctx, Deps>) => HostScope<Name, Ctx, Deps>): PluginContractSurface<Name, Ctx, Deps>;
  lifecycle(fn: (scope: LifecycleScope<Name, Ctx, Deps>) => LifecycleScope<Name, Ctx, Deps>): PluginContractSurface<Name, Ctx, Deps>;
  telemetry(fn: (scope: TelemetryScope<Name, Ctx, Deps>) => TelemetryScope<Name, Ctx, Deps>): PluginContractSurface<Name, Ctx, Deps>;
  build(): BuiltPlugin<Name, Ctx, Deps>;
}
