// see: cans/plugins/replay.md#Plugin-replay
// see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
// see: cans/plugins/replay.md#Deterministic-RNG
// see: cans/plugins/replay.md#Replay-mode-requires-network-isolation-for-determinism

import { kelir } from "@kelir/core";
import { replay } from "@kelir/core";

async function main() {
  const session = await kelir({
    vcpus: 2, memoryMb: 1024, pageSizeKb: 4, driver: "auto",
    kernel: `./fixtures/vmlinux-6.6-${process.arch}-kelir`,
    rootfsErofs: "./fixtures/rootfs-replay.erofs",
    tscLocked: true, enableWatchdog: true,
    deterministicClock: true,
    pinnedClocksource: "kvm-clock",
    networkOff: true,
    nsPerInstruction: 1,
    entropySeed: 0x5EED,
    preemptionGraceMs: 0,
  })
    .use(replay({
      stepWindowInstructions: 1_000_000,
      pinnedClocksource: "kvm-clock",
      deterministicRngSeed: 0x5EED,
      networkIsolation: true,
    }))
    .seal()
    .boot();
  try {
    const timestamps: string[] = [];
    for (let run = 0; run < 2; run++) {
      const slot = await session.checkpoint.create(`replay-run-${run}`);
      console.error(`[kelir] run ${run} -> slot=${slot.id}`);

      const dateRes = await session.spawn("date", ["+%s.%N"]);
      if (dateRes.exitCode !== 0) throw new Error("date spawn failed");
      timestamps.push(new TextDecoder().decode(dateRes.stdout).trim());

      if (run === 0) {
        await session.checkpoint.rollback(slot.id);
      }
    }

    console.error(`[kelir] run 0 date: ${timestamps[0]}`);
    console.error(`[kelir] run 1 date: ${timestamps[1]}`);
    if (timestamps[0] !== timestamps[1]) {
      throw new Error(`clock divergence in deterministic replay: ${timestamps[0]} !== ${timestamps[1]}`);
    }
  } finally {
    await session.stop();
  }
}

main().catch(console.error);
