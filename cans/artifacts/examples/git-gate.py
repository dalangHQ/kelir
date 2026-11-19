# see: cans/02-plugin-model.md#Unified-interposition-pipeline
# see: cans/05-storage-plane.md#Encapsulation-invariant
# see: cans/plugins/redaction.md#Plugin-redaction
from __future__ import annotations

import asyncio
import platform
import re
from pathlib import Path
from typing import Optional

import kelir
from kelir import FilterAction, HandlerContext, KelirBuilder, TrapEvent, Verdict
from kelir.storage_planes import IdmapConfig


PROTECTED = ("main", "master")

_TOKEN_RE = re.compile(rb"sk_live_[A-Za-z0-9]{24}")


# see: cans/plugins/redaction.md#Plugin-redaction
def _mask_fs_write(
    path: Path, data: bytearray, pid: int, ctx: HandlerContext,
) -> FilterAction:
    data[:] = _TOKEN_RE.sub(lambda m: b"*" * len(m.group(0)), bytes(data))
    return FilterAction.forward()


async def _gate_git_trap(event: TrapEvent) -> Optional[Verdict]:
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


def git_gate_plugin() -> KelirBuilder:
    return (
        KelirBuilder.plugin("git-gate")
        .fs(lambda f: f.on_fs_write(_mask_fs_write))
        .trap(lambda t: t.on_trap(_gate_git_trap))
    )


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
            encapsulation="passthrough",
        )
        .use(git_gate_plugin())
        .seal()
    )


async def main() -> None:
    async with build_runtime() as session:
        await session.terminal.resize(120, 40)
        slot = await session.checkpoint.create("git-gate-base")
        print(f"[kelir] checkpoint id={slot.id}")

        # Test 1: Verify force-push mutation stripping
        push = await session.spawn("git", ["push", "--force", "origin", "main"])
        print(f"[kelir] git push -> exit={push.exit_code}")
        assert push.exit_code == 0

        # Test 2: Verify protected branch commit abort
        commit = await session.spawn("git", ["commit", "-m", "direct commit to main"])
        print(f"[kelir] git commit main -> exit={commit.exit_code}")
        assert commit.exit_code != 0, "protected commit should have aborted"

if __name__ == "__main__":
    asyncio.run(main())
