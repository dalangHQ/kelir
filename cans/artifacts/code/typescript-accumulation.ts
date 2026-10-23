// ILLUSTRATIVE EXCERPT — not standalone-executable.
//
// Demonstrates TypeScript SDK's two-phase builder pattern:
// Plugins register via .use(); .configure() receives the fully unified TContext.
//
// Full runnable example: artifacts/examples/autonomous-swe-agent.ts
// Canonical spec home:    cans/03-sdk-architecture.md#TypeScript-SDK

import { kelir, t } from "@kelir/core";
import { vfs } from "@kelir/plugin-vfs";
import { ledger } from "@kelir/plugin-ledger";

const runtime = kelir({ vcpus: 4, memoryMb: 4096 })
  .use(vfs(), { paths: ["/workspace/**"] })
  // 1. Null-object pattern: c.ledger is 100% typed; dispatch table skips hooks at boot
  .use(ledger({ enabled: false }))
  // 2. Mount scoping: filter to target devices and binaries before dispatch evaluation
  .use(repoPolicyPlugin({ maxDiffBytes: 64 * 1024 }), {
    devices: ["virtio-vsock", "virtio-fs"],
    binaries: ["git", "npm"],
  })
  .configure((c) => {
    // 3. Static and runtime schema validation
    .guard({
      schema: t.Object({
        action: t.Literal("fix-webhook"),
        issueId: t.String(),
      }),
    })
    // 4. `c.policy`, `c.fs`, and `c.ledger` are fully inferred on unified context:
    .before("git commit *", async (c) => {
      if (c.policy.violations > 0) {
        return c.deny(1, `EPOLICY: ${c.policy.violations} violation(s)`);
      }
      return c.resume();
    })
  });
