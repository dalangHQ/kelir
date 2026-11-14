# see: cans/02-plugin-model.md#Unified-interposition-pipeline
# see: cans/05-storage-plane.md#Encapsulation-invariant
# see: cans/05-storage-plane.md#In-flight-read-masking
from __future__ import annotations

import asyncio
import platform
from typing import Optional

import kelir
from kelir import KelirBuilder, TrapEvent, Verdict
from kelir.model_plugin import BasePlugin
from kelir.storage_planes import IdmapConfig


PROTECTED = ("main", "master")


class GitGatePlugin(BasePlugin):
    name = "git-gate"

    async def on_trap(self, event: TrapEvent) -> Optional[Verdict]:
        binary = event.binary_path.rsplit("/", 1)[-1]
        if binary != "git":
            return Verdict.pass_()
        if len(event.argv) < 2:
            return Verdict.pass_()
        if event.argv[1] == "push":
            if "--force" in event.argv:
                return Verdict.patch(
                    argv=[a for a in event.argv if a != "--force"],
                    env={"KELIR_DISARMED": "1"},
                )
            return Verdict.pass_()
        if event.argv[1] == "commit":
            if "-m" in event.argv:
                idx = event.argv.index("-m")
                msg = event.argv[idx + 1] if idx + 1 < len(event.argv) else ""
            else:
                msg = ""
            if any(p in msg for p in PROTECTED):
                return Verdict.abort(
                    reason="EPOLICY: commit references protected branch",
                )
            return Verdict.pass_()
        return Verdict.pass_()


def build_runtime() -> KelirBuilder:
    return (
        kelir.builder(
            vcpus=2, memory_mb=1024, driver="auto",
            kernel=f"./fixtures/vmlinux-6.6-{platform.machine()}-kelir",
            rootfs_erofs="./fixtures/rootfs-git.erofs",
            tsc_locked=True, enable_watchdog=True,
        )
        .storage(
            idmap=IdmapConfig(guest_uid=0, host_uid=0),
            shared_caches=[],
            lock_broker=False,
            synthetic_files=[],
            mask_read=[],
            encapsulation="passthrough",
            scan_engine="FastRegex",
            carryover_bytes=1024,
        )
        .use(GitGatePlugin())
        .seal()
    )


async def main() -> None:
    async with build_runtime() as session:
        await session.terminal.resize(120, 40)
        slot = await session.checkpoint.create("git-gate-base")
        print(f"[kelir] checkpoint id={slot.id}")
        # TODO (L3): spawn git push --force origin main in the guest and gate it via the git-gate plugin.


if __name__ == "__main__":
    asyncio.run(main())
