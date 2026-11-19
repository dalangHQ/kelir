// ILLUSTRATIVE, NON-STANDALONE — do not compile this file directly.
// It pins the TypeScript scope interface shapes for the unified builder;
// the builder class lives in cans/artifacts/code/typescript-plugin-builder.ts
// and the normative contract is cans/02-plugin-model.md#Nested-scopes.
//
// Owned by: cans/02-plugin-model.md#Nested-scopes
// see: cans/02-plugin-model.md#Nested-scopes
// see: cans/02-plugin-model.md#Chain-decisions
// see: cans/artifacts/code/typescript-plugin-builder.ts
// see: cans/artifacts/code/scope-builders.rs

import type {
  FilterAction,
  HeartbeatEvent,
  LifecycleContext,
  ProcessExitEvent,
  TrapInfo,
  Verdict,
  WatchdogEvent,
  WatchdogVerdict,
} from "./plugin.model.js";
import type { KelirBuilder, MountScope, ScopeRules } from "./builder.lifecycle.js";

// see: cans/02-plugin-model.md#Nested-scopes
// Scope typing pin, TS leg: every scope is a generic interface. A scope
// without the member a closure calls fails tsc: the FsScope interface
// has no onTrap, so f.onTrap(...) inside .fs(...) is a type error.
// A closure need not call .end(): it returns the scope or void; the
// parent chain resumes after the closure returns. The 8 ring-group
// scopes still declare end(): Owner as an optional terminator in the
// live shapes (packages/ts/core/src/hook.registry.ts); the 5
// orphan-verb scopes have no end().
export interface FsScope<TMode extends "host" | "plugin", TSealed extends "unsealed" | "sealed", Name extends string, Ctx, Deps extends string> {
  onFsLookup(handler: (path: string) => Verdict): FsScope<TMode, TSealed, Name, Ctx, Deps>;
  onFsRead(handler: (path: string, buffer: Uint8Array) => Uint8Array): FsScope<TMode, TSealed, Name, Ctx, Deps>;
  onFsWrite(
    handler: (path: string, buffer: Uint8Array, pid: number, ctx: Ctx) => FilterAction,
  ): FsScope<TMode, TSealed, Name, Ctx, Deps>;
  onFsLock(
    handler: (path: string, flockCmd: number, flockType: number, pid: number) => Verdict,
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
  config(cfg: {
    lockBroker?: unknown;
    carryoverBytes?: number;
    scanEngine?: string;
    snoops?: readonly unknown[];
  }): FsScope<TMode, TSealed, Name, Ctx, Deps>;
  requires<Req extends string>(): FsScope<Name, Ctx, Deps | Req>;
  forkWith(fn: () => KelirBuilder<Name, Ctx, Deps, unknown>): FsScope<TMode, TSealed, Name, Ctx, Deps>;
  timeout(seconds: number): FsScope<TMode, TSealed, Name, Ctx, Deps>;
  priority(i: number): FsScope<TMode, TSealed, Name, Ctx, Deps>;
  rules(rules: ScopeRules): FsScope<TMode, TSealed, Name, Ctx, Deps>;
}

export interface TrapScope<TMode extends "host" | "plugin", TSealed extends "unsealed" | "sealed", Name extends string, Ctx, Deps extends string> {
  onTrap(
    handler: (event: TrapInfo, c: Ctx) => Promise<Verdict | null> | Verdict | null,
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
  requires<Req extends string>(): TrapScope<TMode, TSealed, Name, Ctx, Deps>;
  timeout(seconds: number): TrapScope<TMode, TSealed, Name, Ctx, Deps>;
  priority(i: number): TrapScope<TMode, TSealed, Name, Ctx, Deps>;
}

// see: cans/02-plugin-model.md#Nested-scopes
// The ring scopes are all defined below; each lists only its own ring
// verbs plus requires / timeout / priority. config / forkWith / rules
// are FsScope only (method-home table: cans/02-plugin-model.md#Nested-scopes).
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
  requires<Req extends string>(): NetScope<TMode, TSealed, Name, Ctx, Deps>;
  timeout(seconds: number): NetScope<TMode, TSealed, Name, Ctx, Deps>;
  priority(i: number): NetScope<TMode, TSealed, Name, Ctx, Deps>;
}

export interface ExecScope<TMode extends "host" | "plugin", TSealed extends "unsealed" | "sealed", Name extends string, Ctx, Deps extends string> {
  onExecStdout(handler: (pid: number, execId: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array): ExecScope<TMode, TSealed, Name, Ctx, Deps>;
  onExecStderr(handler: (pid: number, execId: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array): ExecScope<TMode, TSealed, Name, Ctx, Deps>;
  onExecStdin(handler: (pid: number, execId: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array): ExecScope<TMode, TSealed, Name, Ctx, Deps>;
  requires<Req extends string>(): ExecScope<TMode, TSealed, Name, Ctx, Deps>;
  timeout(seconds: number): ExecScope<TMode, TSealed, Name, Ctx, Deps>;
  priority(i: number): ExecScope<TMode, TSealed, Name, Ctx, Deps>;
}

export interface HostScope<TMode extends "host" | "plugin", TSealed extends "unsealed" | "sealed", Name extends string, Ctx, Deps extends string> {
  onHostLsmExec(handler: (path: string, pid: number) => Verdict): HostScope<TMode, TSealed, Name, Ctx, Deps>;
  onHostPressure(handler: (level: number) => Verdict): HostScope<TMode, TSealed, Name, Ctx, Deps>;
  onHostKeyringAccess(handler: (key: string) => Verdict): HostScope<TMode, TSealed, Name, Ctx, Deps>;
  onHostPreempt(handler: (ctx: LifecycleContext) => Promise<void> | void): HostScope<TMode, TSealed, Name, Ctx, Deps>;
  onHostContainmentBreach(handler: (pid: number, path: string, kind: string) => void): HostScope<TMode, TSealed, Name, Ctx, Deps>;
  requires<Req extends string>(): HostScope<TMode, TSealed, Name, Ctx, Deps>;
  timeout(seconds: number): HostScope<TMode, TSealed, Name, Ctx, Deps>;
  priority(i: number): HostScope<TMode, TSealed, Name, Ctx, Deps>;
}

export interface ConsoleScope<TMode extends "host" | "plugin", TSealed extends "unsealed" | "sealed", Name extends string, Ctx, Deps extends string> {
  onConsoleInput(handler: (port: number, bytes: Uint8Array) => Promise<Uint8Array> | Uint8Array): ConsoleScope<TMode, TSealed, Name, Ctx, Deps>;
  onConsoleOutput(handler: (port: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array): ConsoleScope<TMode, TSealed, Name, Ctx, Deps>;
  requires<Req extends string>(): ConsoleScope<TMode, TSealed, Name, Ctx, Deps>;
  timeout(seconds: number): ConsoleScope<TMode, TSealed, Name, Ctx, Deps>;
  priority(i: number): ConsoleScope<TMode, TSealed, Name, Ctx, Deps>;
}

export interface LifecycleScope<TMode extends "host" | "plugin", TSealed extends "unsealed" | "sealed", Name extends string, Ctx, Deps extends string> {
  onVmInit(handler: (ctx: LifecycleContext) => Promise<void> | void): LifecycleScope<TMode, TSealed, Name, Ctx, Deps>;
  onVmReady(handler: (ctx: LifecycleContext) => Promise<void> | void): LifecycleScope<TMode, TSealed, Name, Ctx, Deps>;
  onVmShutdown(handler: (ctx: LifecycleContext) => Promise<void> | void): LifecycleScope<TMode, TSealed, Name, Ctx, Deps>;
  onPreCommit(handler: (ctx: LifecycleContext) => Promise<void> | void): LifecycleScope<TMode, TSealed, Name, Ctx, Deps>;
  onPostCommit(handler: (ctx: LifecycleContext, committed: boolean) => Promise<void> | void): LifecycleScope<TMode, TSealed, Name, Ctx, Deps>;
  requires<Req extends string>(): LifecycleScope<TMode, TSealed, Name, Ctx, Deps>;
  timeout(seconds: number): LifecycleScope<TMode, TSealed, Name, Ctx, Deps>;
  priority(i: number): LifecycleScope<TMode, TSealed, Name, Ctx, Deps>;
}

export interface TelemetryScope<TMode extends "host" | "plugin", TSealed extends "unsealed" | "sealed", Name extends string, Ctx, Deps extends string> {
  onMemoryPressure(handler: (availableMb: number, totalMb: number) => Promise<void> | void): TelemetryScope<TMode, TSealed, Name, Ctx, Deps>;
  onHeartbeat(handler: (event: HeartbeatEvent) => Promise<void> | void): TelemetryScope<TMode, TSealed, Name, Ctx, Deps>;
  onWatchdog(handler: (event: WatchdogEvent) => Promise<WatchdogVerdict> | WatchdogVerdict): TelemetryScope<TMode, TSealed, Name, Ctx, Deps>;
  requires<Req extends string>(): TelemetryScope<TMode, TSealed, Name, Ctx, Deps>;
  timeout(seconds: number): TelemetryScope<TMode, TSealed, Name, Ctx, Deps>;
  priority(i: number): TelemetryScope<TMode, TSealed, Name, Ctx, Deps>;
}

// see: cans/02-plugin-model.md#Nested-scopes
// The five orphan-verb scopes as interfaces: actions, streams, vsock,
// params, deny. Each lists only its own verb plus requires / timeout /
// priority; config / forkWith / rules are FsScope only.

export interface ActionsScope<TMode extends "host" | "plugin", TSealed extends "unsealed" | "sealed", Name extends string, Ctx, Deps extends string> {
  register(
    name: string,
    schema: unknown,
    handler: (payload: unknown) => Promise<void> | void,
  ): ActionsScope<TMode, TSealed, Name, Ctx, Deps>;
  requires<Req extends string>(): ActionsScope<TMode, TSealed, Name, Ctx, Deps>;
  timeout(seconds: number): ActionsScope<TMode, TSealed, Name, Ctx, Deps>;
  priority(i: number): ActionsScope<TMode, TSealed, Name, Ctx, Deps>;
}

export interface StreamsScope<TMode extends "host" | "plugin", TSealed extends "unsealed" | "sealed", Name extends string, Ctx, Deps extends string> {
  register(
    kind: number,
    schema: unknown,
    handler: (request: Uint8Array) => Promise<Uint8Array> | Uint8Array,
  ): StreamsScope<TMode, TSealed, Name, Ctx, Deps>;
  requires<Req extends string>(): StreamsScope<TMode, TSealed, Name, Ctx, Deps>;
  timeout(seconds: number): StreamsScope<TMode, TSealed, Name, Ctx, Deps>;
  priority(i: number): StreamsScope<TMode, TSealed, Name, Ctx, Deps>;
}

export interface VsockScope<TMode extends "host" | "plugin", TSealed extends "unsealed" | "sealed", Name extends string, Ctx, Deps extends string> {
  service(
    port: number,
    service: string,
    handler: (request: unknown) => Promise<unknown> | unknown,
  ): VsockScope<TMode, TSealed, Name, Ctx, Deps>;
  requires<Req extends string>(): VsockScope<TMode, TSealed, Name, Ctx, Deps>;
  timeout(seconds: number): VsockScope<TMode, TSealed, Name, Ctx, Deps>;
  priority(i: number): VsockScope<TMode, TSealed, Name, Ctx, Deps>;
}

export interface ParamsScope<TMode extends "host" | "plugin", TSealed extends "unsealed" | "sealed", Name extends string, Ctx, Deps extends string> {
  model<T>(model: new () => T): ParamsScope<TMode, TSealed, Name, Ctx, Deps>;
  requires<Req extends string>(): ParamsScope<TMode, TSealed, Name, Ctx, Deps>;
  timeout(seconds: number): ParamsScope<TMode, TSealed, Name, Ctx, Deps>;
  priority(i: number): ParamsScope<TMode, TSealed, Name, Ctx, Deps>;
}

export interface DenyScope<TMode extends "host" | "plugin", TSealed extends "unsealed" | "sealed", Name extends string, Ctx, Deps extends string> {
  onTrapDeny(
    handler: (
      event: unknown,
      verdict: Verdict,
      session: unknown,
    ) => Promise<void> | void,
  ): DenyScope<TMode, TSealed, Name, Ctx, Deps>;
  requires<Req extends string>(): DenyScope<TMode, TSealed, Name, Ctx, Deps>;
  timeout(seconds: number): DenyScope<TMode, TSealed, Name, Ctx, Deps>;
  priority(i: number): DenyScope<TMode, TSealed, Name, Ctx, Deps>;
}

// see: cans/02-plugin-model.md#Nested-scopes
// The negative vector f.onTrap(...) inside .fs(...) now lives in
// packages/ts/core/tests/compile-fail/builder.negative.ts as an
// executable expect-error pin; comment-pinned negatives are gone.

// see: cans/02-plugin-model.md#Nested-scopes
// Scope entry point signatures on the builder: each takes a closure
// receiving the scope and returning it or void; a closure need not call
// .end() (ring scopes declare end(): Owner as an optional terminator;
// orphan scopes have none).
declare module "./builder.lifecycle.js" {
  interface KelirBuilder<TMode extends "host" | "plugin", TSealed extends "unsealed" | "sealed", Name extends string, Ctx, Deps extends string, Params> {
    actions(
      fn: (scope: ActionsScope<TMode, TSealed, Name, Ctx, Deps>) => ActionsScope<Name, Ctx, Deps> | void,
    ): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, Params>;
    streams(
      fn: (scope: StreamsScope<TMode, TSealed, Name, Ctx, Deps>) => StreamsScope<Name, Ctx, Deps> | void,
    ): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, Params>;
    vsock(
      fn: (scope: VsockScope<TMode, TSealed, Name, Ctx, Deps>) => VsockScope<Name, Ctx, Deps> | void,
    ): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, Params>;
    params(
      fn: (scope: ParamsScope<TMode, TSealed, Name, Ctx, Deps>) => ParamsScope<Name, Ctx, Deps> | void,
    ): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, Params>;
    deny(
      fn: (scope: DenyScope<TMode, TSealed, Name, Ctx, Deps>) => DenyScope<Name, Ctx, Deps> | void,
    ): KelirBuilder<TMode, TSealed, Name, Ctx, Deps, Params>;
  }
}
