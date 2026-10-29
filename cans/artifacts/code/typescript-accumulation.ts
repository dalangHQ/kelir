// ILLUSTRATIVE EXCERPT — not standalone-executable.
//
// Demonstrates TypeScript SDK's two-phase builder pattern:
// Plugins register via .use(); .params() pushes the session schema,
// AOT-compiled to a native Rust validator at boot; .trap() gates
// execution. SyntheticFile: FUSE-presented paths with no host backing
// file. Verification Reject: c.deny returns a POSIX errno.
//
// Full runnable example: artifacts/examples/autonomous-swe-agent.ts
// Canonical spec home:    cans/03-sdk-architecture.md#TypeScript-SDK

import { kelir, createPlugin, t } from "@kelir/core";
import { ledger } from "@kelir/plugin-ledger";

// 1. Level 3 leaf plugin
const secretMask = createPlugin("secret-mask")
  .decorate("maskActive", () => true);

// 2. Level 2 composite plugin (nests Level 3 + vfs)
const secureWorkspace = createPlugin("secure-workspace")
  .use(secretMask);

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
    const policy = c.service(RepoPolicyService);
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
