// ILLUSTRATIVE EXCERPT — not standalone-executable.
//
// Demonstrates TypeScript SDK's type-level context accumulation:
// removing `.use(repoPolicyPlugin)` strips `policy` from `TContext`,
// and downstream references to `c.policy` fail at `tsc`.
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
  // 2. Mount scoping: filter to target rings and binaries before dispatch evaluation
  .use(repoPolicyPlugin({ maxDiffBytes: 64 * 1024 }), {
    rings: ["virtio-vsock", "virtio-fs"],
    binaries: ["git", "npm"],
  })
  .exec((cmd) => cmd
    // 3. Static and runtime schema validation
    .guard({
      schema: t.Object({
        action: t.Literal("fix-webhook"),
        issueId: t.String(),
      }),
    })
    // 4. `c.policy`, `c.fs`, and `c.ledger` are fully inferred on HandlerContext:
    .before("git commit *", async (c) => {
      if (c.policy.violations > 0) {
        return c.deny(1, `EPOLICY: ${c.policy.violations} violation(s)`);
      }
      return c.resume();
    })
  );
