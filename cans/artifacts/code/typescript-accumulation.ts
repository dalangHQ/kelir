// ILLUSTRATIVE EXCERPT — not standalone-executable.
//
// Demonstrates TypeScript SDK's ElysiaJS-style generic accumulation monad:
// KelirBuilder<TPlugins, TParams> accumulates plugin capabilities at
// compile-time. Each .use(plugin) appends the plugin's context type
// to TPlugins and verifies prerequisite constraints via `requires`.
// Downstream handlers receive HandlerContext<TPlugins, TParams> where
// capabilities (c['repo-policy'], c.secretMask) exist as statically typed members.
//
// Full runnable example: artifacts/examples/autonomous-swe-agent.ts
// Canonical spec home:    cans/03-sdk-architecture.md#TypeScript-SDK

import { kelir, createPlugin, t } from "@kelir/core";
import { ledger } from "@kelir/plugin-ledger";
import type { PluginContract } from "@kelir/core";

// 1. Leaf plugin declaring its static context
const secretMask = createPlugin("secretMask", { maskActive: true });

// 2. Composite plugin enforcing prerequisite on TPlugins
const secureWorkspace = createPlugin("secureWorkspace")
  .requires<typeof secretMask>()
  .derive((c) => ({ workspaceSecured: c.secretMask.maskActive }));

// 3. Accumulation monad: each .use() yields a newly typed builder
const runtime = kelir({ vcpus: 4, memoryMb: 4096 })
  .use(secretMask)
  .use(secureWorkspace)
  .use(ledger({ enabled: false }))
  .use(repoPolicyPlugin({ maxDiffBytes: 64 * 1024 }))
  // Schema push-down: AOT-compiled to native Rust validator at boot
  .params(t.Object({
    action: t.Literal("fix-webhook"),
    issueId: t.String(),
  }))
  // c is fully typed: c['repo-policy'], c.secretMask, c.secureWorkspace exist at compile time
  .trap(TrapFilter.binary("git").withArg("commit"), async (c) => {
    const violations = c["repo-policy"].policy.violations;
    if (violations > 0) {
      return c.drop("EPERM", `EPOLICY: ${violations} violation(s)`);
    }
    return c.pass();
  });

// boot() materializes each .use()'d plugin Context onto the session
// (c['repo-policy'], c.secretMask, ...) before the first handler runs;
// the capability members are not stubs and not populated before boot.
const session = await runtime.boot();
await session.terminal.resize(120, 40);
