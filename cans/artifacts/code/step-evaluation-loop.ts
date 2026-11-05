// ILLUSTRATIVE EXCERPT — not standalone-executable.
//
// Verification step loop using external evaluator.
// Checkpoints and verification state are managed in userland
// orchestrator scope, decoupled from hypervisor microkernel core.
//
// Full runnable example: artifacts/examples/autonomous-swe-agent.ts
// Canonical spec home:    cans/10-attestation.md#Step-evaluation-loop

import { kelir } from "@kelir/core";

const orchestrator = new VerificationEvaluator();
let activeCheckpoint: SnapshotDescriptor | null = null;

const session = await kelir({ vcpus: 2, memoryMb: 2048 })
  .storage({ sentinelPath: "/dev/kelir-gate" })
  .trap("*", {
    pre: async (c) => {
      // 1. Hardware Snapshot: record clean state in orchestrator scope
      activeCheckpoint = await c.vm.checkpoint("pre-step");

      // 2. Hardware Pause: hold vCPU registers while verifying intent
      const intent = await orchestrator.analyzeIntent({
        binary: c.trap.binary_path,
        argv: c.trap.argv,
        lineage: c.trap.ppid,
      });

      if (!intent.isHypothesisValid) {
        // Verdict::Drop { errno, reason }: EPERM is the generic POSIX
        // denial used here; no verification-specific errno is pinned
        // in the grounded spec facts. errno None would mean silent
        // blackhole, chaos loss only.
        return c.drop("EPERM", intent.falsificationReason);
      }

      return c.pass();
    },
    post: async (c) => {
      // 3. Post-Condition Verification via Tier 1 Hardware Evidence
      const receipt = await c.receipt();
      if (!await orchestrator.verifyPhysicalReceipt(receipt)) {
        // 4. Instant Hardware Rollback: memory reverts via driver rollback
        // and the broker drops the upper file and tombstones, base
        // untouched (FALLOC_FL_PUNCH_HOLE + posix_fadvise(DONTNEED));
        // rollback also closes host sockets, guest sees ECONNRESET/EPIPE.
        if (activeCheckpoint) await c.vm.rollback(activeCheckpoint);
        session.terminal.notice("⚡ [VERIFICATION ENGINE]: Reverted.\n");
      }
    },
  })
  .boot();

await session.spawn("claude", ["code", "run test verification"]);
