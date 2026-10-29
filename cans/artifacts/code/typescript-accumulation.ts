// ILLUSTRATIVE EXCERPT — not standalone-executable.
//
// Demonstrates TypeScript SDK's two-phase builder pattern:
// Plugins register via .use(), which intersects TPluginCtx into TContext
// and unions TPluginId into TInstalledPlugins. A missing prerequisite
// yields CompileError<...>; the returned builder has no callable method.
// .params(schema) pins TParams = Static<TSchema>. Handler capabilities are
// static members (c.policy), never c.service(Token).
//
// Full runnable example: artifacts/examples/autonomous-swe-agent.ts
// Canonical spec home:    cans/03-sdk-architecture.md#TypeScript-SDK

import { kelir, createPlugin, t } from "@kelir/core";
import { ledger } from "@kelir/plugin-ledger";
import type { PluginContract } from "@kelir/core";

// 1. Level 3 leaf plugin — declares its static context shape
const secretMask: PluginContract<
  "secret-mask",
  { maskActive: boolean }
> = createPlugin("secret-mask", { maskActive: true });

// 2. Level 2 composite plugin — prerequisite declared in the type
const secureWorkspace: PluginContract<
  "secure-workspace",
  { maskActive: boolean },
  "secret-mask"
> = createPlugin("secure-workspace").requires(secretMask);

// 3. Level 1 runtime builder mounts composite
const runtime = kelir({ vcpus: 4, memoryMb: 4096 })
  .use(secureWorkspace)
  // Null-object pattern: disabled plugin preserves service interfaces; dispatch skips hooks at boot
  .use(ledger({ enabled: false }))
  .use(repoPolicyPlugin({ maxDiffBytes: 64 * 1024 }))
  // Schema push-down: AOT-compiled to native Rust validator at boot
  .params(t.Object({
    action: t.Literal("fix-webhook"),
    issueId: t.String(),
  }))
  // Nano-tracer lifecycle tap
  .trace((t) => {
    if (t.ffi_hop_ns > 10_000) console.warn("FFI boundary spike:", t);
  })
  // Services resolved via typed service locator; no ambient string grab-bags
  .trap(TrapFilter.binary("git").withArg("commit"), async (c) => {
    const policy = c.policy;
    if (policy.violations > 0) {
      return c.deny(1, `EPOLICY: ${policy.violations} violation(s)`);
    }
    return c.resume();
  })
  .on(LifecycleEvent.VmReady, async (c) => {
    return c.resume();
  });

// Native engine planes directly on Session.
const session = await runtime.boot();
await session.terminal.resize(120, 40);
