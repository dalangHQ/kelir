// ILLUSTRATIVE, NON-STANDALONE — do not compile this file directly.
// It pins the TypeScript fluent plugin-creation builder surface; the live
// shape is packages/ts/core/src/plugin.model.ts (Agent 4) and the normative
// contract is cans/02-plugin-model.md#Fluent-plugin-creation-builder.
//
// Owned by: cans/02-plugin-model.md#Fluent-plugin-creation-builder
// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
// see: cans/03-sdk-architecture.md#Fluent-plugin-surface

import { createPlugin } from "./plugin.model.js";
import type { KelirPlugin, PluginContract } from "./plugin.model.js";

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export type PluginBuilder<Ctx = unknown, Deps extends string = never> =
  PluginContract<string, Ctx, Deps>;

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
export function buildChaosPlugin(): KelirPlugin {
  return createPlugin("chaos")
    .context<Record<string, unknown>>(() => ({}))
    .scope({ paths: ["/workspace"] })
    .onTrap((event, _c) => {
      void event;
      void _c;
      return "Pass";
    })
    .onFsWrite((path, data, _pid, _ctx) => {
      void path;
      void data;
      void _pid;
      void _ctx;
      return "Forward";
    })
    .build();
}

// see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
// Chain shape (live verbs live on PluginContract in plugin.model.ts):
//   createPlugin(name)
//     .context<Ctx>(fn)        — explicit context; inference via createPlugin<Name, Ctx>() is the alternative Agent 4 pins
//     .requires<P>()           — accumulates a Deps union; .use(plugin) rejects a Deps not in caller Ctx
//     .scope(scope)            — default mount scope for every bound ring (literal shape pinned by Agent 4)
//     .forkWith(fn)            — custom fork_state; without it the wrapper re-initializes state on fork
//     .onTrap(fn)              — RingEvent.Trap, ordinal assigned in chain order
//     .onFsWrite(fn)           — RingEvent.FsWrite; .onFsLookup / .onFsRead / .onFsLock likewise
//     .onNetOutbound(fn)       — RingEvent.NetOutbound; .onNetInbound likewise
//     .onConsoleInput(fn)      — RingEvent.ConsoleInput; .onConsoleOutput / .onExecStdout / .onExecStderr likewise
//     .onHostLsmExec(fn)       — RingEvent lifecycle-adjacent host ring; .onHostPressure / .onHostKeyringAccess likewise
//     .onVmInit(fn)            — lifecycle tag "vm-init"; .onVmReady / .onVmShutdown / .onPreCommit / .onPostCommit / .onHostPreempt likewise
//     .onMemoryPressure(fn)    — telemetry ring; .onHeartbeat / .onWatchdog likewise
//     .vsockService(spec, fn)  — RingEvent.VsockService
//     .build()                 — terminal verb; returns a frozen KelirPlugin
//
// Post-.build() behaviour: .build() returns KelirPlugin, not the builder, so
// a further verb call is a method-not-found type error; calling a verb on the
// built object at runtime throws TypeError. .use() accepts KelirPlugin
// (post-build) only, never the pre-build PluginContract.
