# see: cans/plugins/replay.md#Plugin-replay
# see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
# see: cans/plugins/replay.md#Deterministic-RNG
# see: cans/plugins/replay.md#Replay-mode-requires-network-isolation-for-determinism
from __future__ import annotations

import asyncio
import platform

import kelir
from kelir import KelirBuilder
from kelir.plugins.plugin_replay import ReplayConfig, ReplayPlugin


def build_runtime() -> KelirBuilder:
    return (
        kelir.builder(
            vcpus=2, memory_mb=1024, page_size_kb=4, driver="auto",
            kernel=f"./fixtures/vmlinux-6.6-{platform.machine()}-kelir",
            rootfs_erofs="./fixtures/rootfs-replay.erofs",
            tsc_locked=True, enable_watchdog=True,
            deterministic_clock=True,
            pinned_clocksource="kvm-clock",
            preemption_grace_ms=0,
            # TODO: network isolation, ns-per-instruction, and the entropy seed ride the replay plugin config here; kelir.builder has no network_off, ns_per_instruction, or entropy_seed fields.
        )
        .use(ReplayPlugin(ReplayConfig(
            step_window=1_000_000,
            kernel_params=[
                "clocksource=kvm-clock",
                "tsc=reliable",
                "nohpet",
            ],
            deterministic_rng_seed=0x5EED,
            network_isolation=True,
        )))
        .seal()
    )


async def main() -> None:
    async with build_runtime() as session:
        for run in range(2):
            slot = await session.checkpoint.create(f"replay-run-{run}")
            print(f"[kelir] run {run} -> slot={slot.id}")
        # TODO (L3): spawn date +%s.%N in the guest twice and replay both runs deterministically.


if __name__ == "__main__":
    asyncio.run(main())
