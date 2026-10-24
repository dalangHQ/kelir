// ILLUSTRATIVE EXCERPT — not standalone-executable.
//
// Verification step loop using external evaluator.
// Checkpoints and verification state are managed in userland
// orchestrator scope, decoupled from hypervisor microkernel core.
//
// Full runnable example: artifacts/examples/autonomous-swe-agent.ts
// Canonical spec home:    cans/08-epistemic.md#Step-evaluation-loop

import { kelir } from "@kelir/core";
import { vfs } from "@kelir/plugin-vfs";
import { pty } from "@kelir/plugin-pty";

const orchestrator = new VerificationEvaluator();
let activeCheckpoint: SnapshotDescriptor | null = null;

const session = await kelir({ vcpus: 2, memoryMb: 2048 })
  .use(vfs())
  .use(pty())
  .exec((cmd) =>
    cmd
      .before("*", async (c) => {
        // 1. Hardware Snapshot: record clean state in orchestrator scope
        activeCheckpoint = await c.vm.checkpoint("pre-step");

        // 2. Hardware Pause: hold vCPU registers while verifying intent
        const intentAnalysis = await orchestrator.analyzeIntent({
          binary: c.trap.binary_path,
          argv: c.trap.argv,
          lineage: c.trap.ancestor_chain,
        });

        if (!intentAnalysis.isHypothesisValid) {
          return c.deny(1, `Verification Reject: ${intentAnalysis.falsificationReason}`);
        }

        return c.resume();
      })
      .after("*", async (c) => {
        // 3. Post-Condition Verification via Tier 1 Hardware Evidence
        const receipt = await c.receipt();
        const isConsistent = await orchestrator.verifyPhysicalReceipt(receipt);

        if (!isConsistent) {
          // 4. Instant Hardware Rollback: memory and disk revert in <8ms
          if (activeCheckpoint) await c.vm.rollback(activeCheckpoint);
          c.term.notice("⚡ [VERIFICATION ENGINE]: Side-effect violation. State reverted.\n");
        }
      })
  )
  .boot();

await session.spawn("claude", ["code", "run test verification"]);
