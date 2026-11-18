# see: cans/plugins/replay.md#Plugin-replay
# see: cans/plugins/replay.md#Cycle-exact-instruction-clock-stepping
# see: cans/plugins/replay.md#Deterministic-RNG
# see: cans/plugins/replay.md#Replay-mode-requires-network-isolation-for-determinism
from __future__ import annotations

import asyncio
import platform

import kelir
from kelir import KelirBuilder
from kelir.plugins.replay import ReplayConfig, ReplayPlugin


def build_runtime() -> KelirBuilder:
    return (
        kelir.builder(
            vcpus=2, memory_mb=1024, page_size_kb=4, driver="auto",
            kernel=f"./fixtures/vmlinux-6.6-{platform.machine()}-kelir",
            rootfs_erofs="./fixtures/rootfs-replay.erofs",
            tsc_locked=True, enable_watchdog=True,
            deterministic_clock=True,
            pinned_clocksource="kvm-clock",
            network_off=True,
            ns_per_instruction=1,
            entropy_seed=0x5EED,
            preemption_grace_ms=0,
        )
        .use(ReplayPlugin(ReplayConfig(
            step_window_instructions=1_000_000,
            pinned_clocksource="kvm-clock",
            deterministic_rng_seed=0x5EED,
            network_isolation=True,
        )))
        .seal()
    )


async def main() -> None:
    async with build_runtime() as session:
        timestamps = []
        for run in range(2):
            slot = await session.checkpoint.create(f"replay-run-{run}")
            print(f"[kelir] run {run} -> slot={slot.id}")

            res = await session.spawn("date", ["+%s.%N"])
            assert res.exit_code == 0
            timestamps.append(res.stdout.decode().strip())

            if run == 0:
                await session.checkpoint.rollback(slot.id)

        print(f"[kelir] run 0 date: {timestamps[0]}")
        print(f"[kelir] run 1 date: {timestamps[1]}")
        assert timestamps[0] == timestamps[1], "replay clock desynchronization detected"


if __name__ == "__main__":
    asyncio.run(main())
