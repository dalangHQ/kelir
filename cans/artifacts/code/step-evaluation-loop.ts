// ILLUSTRATIVE EXCERPT — not standalone-executable.
//
// The atomic step-evaluation loop used by epistemic orchestrators.
// Replaces language-level monkeypatching with hardware-boundary
// checkpoint / verify / rollback. c.step is a core-provided StepSlot
// on HandlerCtx — no userland scratchpad.
//
// Full runnable example: artifacts/examples/autonomous-swe-agent.ts
// Canonical spec home:    cans/08-epistemic.md#Step-evaluation-loop

import { kelir } from "@kelir/core";
import { vfs } from "@kelir/plugin-vfs";
import { pty } from "@kelir/plugin-pty";

const orchestrator = new EpistemicEvaluator();

const session = await kelir({ vcpus: 2, memoryMb: 2048 })
  .use(vfs())
  .use(pty())
  .exec((cmd) =>
    cmd
      .before("*", async (c) => {
        // 1. Hardware Snapshot: record clean state in a core-owned StepSlot
        await c.step.mark(await c.vm.checkpoint("pre-step"));

        // 2. Hardware Pause: hold vCPU registers while verifying intent
        const intentAnalysis = await orchestrator.analyzeIntent({
          binary: c.trap.binary_path,
          argv: c.trap.argv,
          lineage: c.trap.ancestor_chain,
        });

        if (!intentAnalysis.isHypothesisValid) {
          return c.deny(1, `Epistemic Reject: ${intentAnalysis.falsificationReason}`);
        }

        return c.resume();
      })
      .after("*", async (c) => {
        // 3. Post-Condition Verification via Tier 1 Physical Ground Truth
        const receipt = await c.receipt();
        const isConsistent = await orchestrator.verifyPhysicalReceipt(receipt);

        if (!isConsistent) {
          // 4. Instant Hardware Rollback: memory and disk revert in <8ms
          await c.vm.rollback(c.step.last());
          c.term.notice("⚡ [EPISTEMIC ENGINE]: Side-effect violation. State reverted.\n");
        }
      })
  )
  .boot();

await session.spawn("claude", ["code", "run test verification"]);
