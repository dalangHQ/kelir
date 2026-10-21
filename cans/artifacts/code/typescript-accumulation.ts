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

const runtime = kelir({ vcpus: 4, memoryMb: 4096 })
  .use(vfs())
  .use(repoPolicyPlugin({ maxDiffBytes: 64 * 1024 }))
  .exec((cmd) => cmd
    // 1. Static and runtime schema validation
    .guard({
      schema: t.Object({
        action: t.Literal("fix-webhook"),
        issueId: t.String(),
      }),
    })
    // 2. `c.policy` and `c.fs` are fully inferred on HandlerContext:
    .before("git commit *", async (c) => {
      if (c.policy.violations > 0) {
        return c.deny(1, `EPOLICY: ${c.policy.violations} violation(s)`);
      }
      return c.resume();
    })
  );
