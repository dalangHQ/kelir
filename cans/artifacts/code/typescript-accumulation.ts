// ILLUSTRATIVE EXCERPT — not standalone-executable.
//
// Demonstrates TypeScript SDK's flat builder pattern:
// .use() appends to an ordered registry and returns this (typed
// overload), so handler capabilities are typed members (c.policy),
// never c.service(Token). A missing prerequisite throws EConfig at
// builder.boot(), before vCPU launch. Null-object
// use(plugin({ enabled: false })) validates at boot but skips
// registration. .params(schema) pins TParams = Static<TSchema>.
// No generic type-parameter accumulation.
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
  // Null-object: validates at boot but skips registration
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
  // Verdict methods: c.pass() / c.patch() / c.drop(errno, reason) /
  // c.suspend(until) / c.abort(reason). Drop errno None = silent
  // blackhole, chaos loss only; a policy denial carries an errno
  // (EPERM used here as the generic POSIX denial — no policy-specific
  // errno is pinned in the grounded spec facts).
  .trap(TrapFilter.binary("git").withArg("commit"), async (c) => {
    const policy = c.policy;
    if (policy.violations > 0) {
      return c.drop("EPERM", `EPOLICY: ${policy.violations} violation(s)`);
    }
    return c.pass();
  })
  .on(LifecycleEvent.VmReady, async (c) => {
    return c.pass();
  });

// Native engine planes directly on Session.
const session = await runtime.boot();
await session.terminal.resize(120, 40);
