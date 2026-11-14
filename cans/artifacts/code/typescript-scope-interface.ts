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
// There is no .end() in TypeScript: the closure returns the scope or
// void; the parent chain resumes after the closure returns.
export interface FsScope<Name extends string, Ctx, Deps extends string> {
  onFsLookup(handler: (path: string) => Verdict): FsScope<Name, Ctx, Deps>;
  onFsRead(handler: (path: string, buffer: Uint8Array) => Uint8Array): FsScope<Name, Ctx, Deps>;
  onFsWrite(
    handler: (path: string, buffer: Uint8Array, pid: number, ctx: Ctx) => FilterAction,
  ): FsScope<Name, Ctx, Deps>;
  onFsLock(
    handler: (path: string, flockCmd: number, flockType: number, pid: number) => Verdict,
  ): FsScope<Name, Ctx, Deps>;
  config(cfg: {
    lockBroker?: unknown;
    carryoverBytes?: number;
    scanEngine?: string;
    snoops?: readonly unknown[];
  }): FsScope<Name, Ctx, Deps>;
  requires<Req extends string>(): FsScope<Name, Ctx, Deps | Req>;
  forkWith(fn: () => KelirBuilder<Name, Ctx, Deps, unknown>): FsScope<Name, Ctx, Deps>;
  timeout(seconds: number): FsScope<Name, Ctx, Deps>;
  priority(i: number): FsScope<Name, Ctx, Deps>;
  rules(rules: ScopeRules): FsScope<Name, Ctx, Deps>;
}

export interface TrapScope<Name extends string, Ctx, Deps extends string> {
  onTrap(
    handler: (event: TrapInfo, c: Ctx) => Promise<Verdict | null> | Verdict | null,
  ): TrapScope<Name, Ctx, Deps>;
  onProcessExit(
    handler: (event: ProcessExitEvent) => Promise<void> | void,
  ): TrapScope<Name, Ctx, Deps>;
  config(cfg: unknown): TrapScope<Name, Ctx, Deps>;
  requires<Req extends string>(): TrapScope<Name, Ctx, Deps>;
  forkWith(fn: () => KelirBuilder<Name, Ctx, Deps, unknown>): TrapScope<Name, Ctx, Deps>;
  timeout(seconds: number): TrapScope<Name, Ctx, Deps>;
  priority(i: number): TrapScope<Name, Ctx, Deps>;
  rules(rules: ScopeRules): TrapScope<Name, Ctx, Deps>;
}

// see: cans/02-plugin-model.md#Nested-scopes
// The five orphan-verb scopes as interfaces: actions, streams, vsock,
// params, deny. Ring scopes (net, console, exec, host, lifecycle,
// telemetry) mirror the two above; each lists only its own ring verbs
// plus the shared sub-nodes.
export interface ActionsScope<Name extends string, Ctx, Deps extends string> {
  register(
    name: string,
    schema: unknown,
    handler: (payload: unknown) => Promise<void> | void,
  ): ActionsScope<Name, Ctx, Deps>;
}

export interface StreamsScope<Name extends string, Ctx, Deps extends string> {
  register(
    kind: number,
    schema: unknown,
    handler: (request: Uint8Array) => Promise<Uint8Array> | Uint8Array,
  ): StreamsScope<Name, Ctx, Deps>;
}

export interface VsockScope<Name extends string, Ctx, Deps extends string> {
  service(
    port: number,
    service: string,
    handler: (request: unknown) => Promise<unknown> | unknown,
  ): VsockScope<Name, Ctx, Deps>;
}

export interface ParamsScope<Name extends string, Ctx, Deps extends string> {
  model<T>(model: new () => T): ParamsScope<Name, Ctx, Deps>;
}

export interface DenyScope<Name extends string, Ctx, Deps extends string> {
  onTrapDeny(
    handler: (
      event: unknown,
      verdict: Verdict,
      session: unknown,
    ) => Promise<void> | void,
  ): DenyScope<Name, Ctx, Deps>;
}

// see: cans/02-plugin-model.md#Nested-scopes
// The negative vector: f.onTrap(...) inside .fs(...) is a type error in
// TypeScript. Uncommenting the body must fail tsc with
// "Property 'onTrap' does not exist on type 'FsScope<...>'".
// vectors.json cannot express an expected-error case (its schema
// carries event/expected_verdict only), so the negative case is pinned
// here and in the example file as a comment-pinned block.
//
// export function negativeFsOnTrap(
//   builder: KelirBuilder<"surface", unknown, never, unknown>,
// ): KelirBuilder<"surface", unknown, never, unknown> {
//   return builder.fs((f) => f.onTrap((event) => Verdict.Pass));
// }

// see: cans/02-plugin-model.md#Nested-scopes
// Scope entry point signatures on the builder: each takes a closure
// receiving the scope and returning it or void; there is no end().
declare module "./builder.lifecycle.js" {
  interface KelirBuilder<Name extends string, Ctx, Deps extends string, Params> {
    actions(
      fn: (scope: ActionsScope<Name, Ctx, Deps>) => ActionsScope<Name, Ctx, Deps> | void,
    ): KelirBuilder<Name, Ctx, Deps, Params>;
    streams(
      fn: (scope: StreamsScope<Name, Ctx, Deps>) => StreamsScope<Name, Ctx, Deps> | void,
    ): KelirBuilder<Name, Ctx, Deps, Params>;
    vsock(
      fn: (scope: VsockScope<Name, Ctx, Deps>) => VsockScope<Name, Ctx, Deps> | void,
    ): KelirBuilder<Name, Ctx, Deps, Params>;
    params(
      fn: (scope: ParamsScope<Name, Ctx, Deps>) => ParamsScope<Name, Ctx, Deps> | void,
    ): KelirBuilder<Name, Ctx, Deps, Params>;
    deny(
      fn: (scope: DenyScope<Name, Ctx, Deps>) => DenyScope<Name, Ctx, Deps> | void,
    ): KelirBuilder<Name, Ctx, Deps, Params>;
  }
}
