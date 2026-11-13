// see: cans/plugins/replay.md#Plugin-replay
// see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
// see: cans/plugins/replay.md#Deterministic-RNG
// see: cans/plugins/replay.md#Replay-mode-requires-network-isolation-for-determinism

import { kelir } from "@kelir/core";
import { replay } from "@kelir/core";

const runtime = kelir({
  vcpus: 2, memoryMb: 1024, pageSizeKb: 4, driver: "auto",
  kernel: `./fixtures/vmlinux-6.6-${process.arch}-kelir`,
  rootfsErofs: "./fixtures/rootfs-replay.erofs",
  tscLocked: true, enableWatchdog: true,
  deterministicClock: true,
  pinnedClocksource: "kvm-clock",
  preemptionGraceMs: 0,
  // TODO: network isolation, ns-per-instruction, and the entropy seed ride the replay plugin config here; KelirConfig has no networkOff, nsPerInstruction, or entropySeed fields.
})
  .use(replay({
    stepWindow: 1_000_000,
    kernelParams: [
      "clocksource=kvm-clock",
      "tsc=reliable",
      "nohpet",
    ],
    deterministicRngSeed: 0x5EED,
    networkIsolation: true,
  }));

async function main() {
  runtime.seal();
  const session = await runtime.boot();
  try {
    for (let run = 0; run < 2; run++) {
      const slot = await session.checkpoint.create(`replay-run-${run}`);
      console.error(`[kelir] run ${run} -> slot=${slot.id}`);
    }
    // TODO (L3): spawn date +%s.%N in the guest twice and replay both runs deterministically.
  } finally {
    await session.stop();
  }
}

main().catch(console.error);
