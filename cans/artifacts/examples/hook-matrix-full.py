# see: cans/02-plugin-model.md#Full-Verdict-lattice
# see: cans/02-plugin-model.md#TrapFilter-predicates
# see: cans/plugins/redaction.md#Plugin-redaction
# see: cans/plugins/terminal-guard.md#Plugin-terminal-guard
from __future__ import annotations

import asyncio
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Optional

import kelir
from kelir import Approval, Deadline, Errno, FilterAction, KelirBuilder, PatchPayload, TrapFilter, Verdict
from kelir.plugin_model import (
    FsInodeAttr,
    HandlerContext,
    WatchdogEvent,
    WatchdogVerdict,
)


IDENTITY_PATH = "/workspace/.policy/identity"
IDENTITY_CONTENT = b"policy-matrix-identity\n"
DENY_MARKER = b"198.51.100."
TOKEN_PREFIX = b"sk_live_"
TOKEN_LEN = len(TOKEN_PREFIX) + 24
CARRYOVER = TOKEN_LEN - 1

_TOKEN_RE = re.compile(rb"sk_live_[A-Za-z0-9]{24}")
_OSC52_RE = re.compile(rb"\x1b\]52;[^\x07\x1b]*(?:\x07|\x1b\\)?")


def escapes_workspace(path: str) -> bool:
    parts = [p for p in path.split("/") if p not in ("", ".")]
    if ".." not in parts:
        return False
    stack: list[str] = []
    for part in parts:
        if part == "..":
            if stack:
                stack.pop()
        else:
            stack.append(part)
    return not stack or stack[0] != "workspace"


def mask_tokens(window: bytes) -> bytes:
    return _TOKEN_RE.sub(lambda m: b"*" * len(m.group(0)), window)


@dataclass
class HookState:
    tail: bytes = b""
    console_tail: bytes = b""
    heartbeats: int = 0
    last_uptime_ms: Optional[int] = None
    stalls: int = 0
    exits: int = 0
    failures: int = 0
    traps: int = 0
    fs_locks: int = 0
    fs_unlinks: int = 0
    fs_renames: int = 0
    net_inbound: int = 0
    exec_stdout: int = 0
    exec_stderr: int = 0
    host_preempts: int = 0
    vm_inits: int = 0
    vm_readys: int = 0
    vm_shutdowns: int = 0
    pre_commits: int = 0
    post_commits: int = 0
    memory_pressures: int = 0


STATE = HookState()


async def _on_trap(event: Any) -> Optional[Verdict]:
    STATE.traps += 1
    if getattr(event, "tree_depth", 0) > 16:
        return Verdict.abort(reason="fork bomb mitigation: max depth exceeded")
    return Verdict.pass_()


async def _on_process_exit(event: Any) -> None:
    STATE.exits += 1
    if event.exit_code != 0:
        STATE.failures += 1


async def _on_signal(sender: int, target: int, sig: int) -> Verdict:
    if target == 1 and sender != 1:
        return Verdict.drop(Errno.EPERM, "EPOLICY: signaling kelir-shim PID 1 denied")
    return Verdict.pass_()


async def _on_fork(parent: int, child: int) -> Verdict:
    return Verdict.pass_()


# see: cans/07-wire-plane.md#Wire-plane
def _on_net_outbound(frame: bytearray) -> FilterAction:
    if DENY_MARKER in frame:
        return FilterAction.suspend(until=Deadline(0))
    return FilterAction.forward()


def _on_net_inbound(frame: bytearray) -> FilterAction:
    STATE.net_inbound += 1
    return FilterAction.forward()


# see: cans/05-storage-plane.md#Storage-plane
# see: cans/02-plugin-model.md#Unified-verdict
def _on_fs_lookup(path: Path) -> Verdict:
    if escapes_workspace(str(path)):
        # Errno is the closed enum; PathTraversal maps to EPERM in the driver.
        return Verdict.drop(
            Errno.PATH_TRAVERSAL,
            "lookup escapes the /workspace root",
        )
    if str(path) != IDENTITY_PATH:
        return Verdict.pass_()
    return Verdict.patch(FsInodeAttr(
        mode=0o400,
        uid=0,
        gid=0,
        size=len(IDENTITY_CONTENT),
        content=IDENTITY_CONTENT,
    ))


# see: cans/plugins/redaction.md#Plugin-redaction
def _on_fs_read(path: Path, buffer: bytes) -> bytes:
    window = STATE.tail + buffer
    masked = mask_tokens(window)
    chunk = masked[len(masked) - len(buffer):] if buffer else b""
    STATE.tail = masked[-CARRYOVER:] if masked else b""
    return chunk


def _on_fs_write(
    path: Path, data: bytearray, pid: int, ctx: HandlerContext,
) -> FilterAction:
    data[:] = mask_tokens(bytes(data))
    return FilterAction.forward()


def _on_fs_lock(
    path: Path, flock_cmd: int, flock_type: int, pid: int,
) -> Verdict:
    STATE.fs_locks += 1
    return Verdict.pass_()


def _on_fs_unlink(path: Path, ctx: Any) -> Verdict:
    STATE.fs_unlinks += 1
    path_str = str(path)
    if path_str.endswith(".protected_source"):
        return Verdict.suspend(until=Approval())
    if path_str.endswith(".trashable"):
        filename = path.name
        return Verdict.patch(PatchPayload.fs_path_redirect(
            target_path=f"/workspace/.agentsh_trash/{filename}"
        ))
    return Verdict.pass_()


def _on_fs_rename(src: Path, dst: Path, ctx: Any) -> Verdict:
    STATE.fs_renames += 1
    if escapes_workspace(str(dst)):
        return Verdict.drop(errno=Errno.PATH_TRAVERSAL, reason="rename escapes /workspace")
    return Verdict.pass_()


def _on_fs_setattr(path: Path, mode: int, size: int) -> Verdict:
    return Verdict.pass_()


def _on_fs_symlink(target: Path, link: Path) -> Verdict:
    if escapes_workspace(str(target)) or escapes_workspace(str(link)):
        return Verdict.drop(Errno.PATH_TRAVERSAL, "symlink escapes /workspace")
    return Verdict.pass_()


def _on_fs_mkdir(path: Path, mode: int) -> Verdict:
    return Verdict.pass_()


def _on_fs_readdir(path: Path, entries: list) -> FilterAction:
    return FilterAction.forward()


def _on_fs_readlink(path: Path) -> Verdict:
    return Verdict.pass_()


def _on_host_lsm_exec(path: Path, pid: int) -> Verdict:
    return Verdict.pass_()


def _on_host_pressure(level: int) -> Verdict:
    return Verdict.pass_()


def _on_host_keyring_access(key: str) -> Verdict:
    return Verdict.pass_()


async def _on_host_preempt(ctx: Any) -> None:
    STATE.host_preempts += 1


def _on_host_containment_breach(pid: int, path: Path, kind: str) -> None:
    pass


async def _on_vm_init(ctx: Any) -> None:
    STATE.vm_inits += 1


async def _on_vm_ready(ctx: Any) -> None:
    STATE.vm_readys += 1


async def _on_vm_shutdown(ctx: Any) -> None:
    STATE.vm_shutdowns += 1


async def _on_pre_commit(ctx: Any) -> None:
    STATE.pre_commits += 1


async def _on_post_commit(ctx: Any, committed: bool) -> None:
    STATE.post_commits += 1


# see: cans/plugins/terminal-guard.md#Plugin-terminal-guard
async def _on_console_input(port: int, data: bytes) -> bytes:
    return _OSC52_RE.sub(b"", data)


# see: cans/plugins/terminal-guard.md#Plugin-terminal-guard
async def _on_console_output(port: int, chunk: bytes) -> bytes:
    window = STATE.console_tail + chunk
    masked = mask_tokens(window)
    out = masked[len(masked) - len(chunk):] if chunk else b""
    STATE.console_tail = masked[-CARRYOVER:] if masked else b""
    return out


async def _on_exec_stdout(pid: int, exec_id: int, chunk: bytes) -> bytes:
    STATE.exec_stdout += 1
    return mask_tokens(chunk)


async def _on_exec_stderr(pid: int, exec_id: int, chunk: bytes) -> bytes:
    STATE.exec_stderr += 1
    return mask_tokens(chunk)


async def _on_exec_stdin(pid: int, exec_id: int, chunk: bytes) -> bytes:
    return chunk


async def _on_memory_pressure(available_mb: int, total_mb: int) -> None:
    STATE.memory_pressures += 1


async def _on_heartbeat(event: Any) -> None:
    if (STATE.last_uptime_ms == event.uptime_ms
            and STATE.heartbeats > 0):
        STATE.stalls += 1
    STATE.last_uptime_ms = event.uptime_ms
    STATE.heartbeats += 1


async def _on_watchdog(event: WatchdogEvent) -> WatchdogVerdict:
    return WatchdogVerdict.ExecuteDefault


def hook_matrix_full_plugin() -> KelirBuilder:
    return (
        KelirBuilder.plugin("hook-matrix-full")
        .trap(
            lambda t: t.on_trap(_on_trap)
            .on_process_exit(_on_process_exit)
            .on_signal(_on_signal)
            .on_fork(_on_fork)
        )
        .fs(
            lambda f: f.on_fs_lookup(_on_fs_lookup)
            .on_fs_read(_on_fs_read)
            .on_fs_write(_on_fs_write)
            .on_fs_lock(_on_fs_lock)
            .on_fs_unlink(_on_fs_unlink)
            .on_fs_rename(_on_fs_rename)
            .on_fs_setattr(_on_fs_setattr)
            .on_fs_symlink(_on_fs_symlink)
            .on_fs_mkdir(_on_fs_mkdir)
            .on_fs_readdir(_on_fs_readdir)
            .on_fs_readlink(_on_fs_readlink)
        )
        .net(
            lambda n: n.on_net_outbound(_on_net_outbound)
            .on_net_inbound(_on_net_inbound)
        )
        .console(
            lambda c: c.on_console_input(_on_console_input)
            .on_console_output(_on_console_output)
        )
        .exec(
            lambda e: e.on_exec_stdout(_on_exec_stdout)
            .on_exec_stderr(_on_exec_stderr)
            .on_exec_stdin(_on_exec_stdin)
        )
        .host(
            lambda h: h.on_host_lsm_exec(_on_host_lsm_exec)
            .on_host_pressure(_on_host_pressure)
            .on_host_keyring_access(_on_host_keyring_access)
            .on_host_preempt(_on_host_preempt)
            .on_host_containment_breach(_on_host_containment_breach)
        )
        .lifecycle(
            lambda l: l.on_vm_init(_on_vm_init)
            .on_vm_ready(_on_vm_ready)
            .on_vm_shutdown(_on_vm_shutdown)
            .on_pre_commit(_on_pre_commit)
            .on_post_commit(_on_post_commit)
        )
        .telemetry(
            lambda tm: tm.on_memory_pressure(_on_memory_pressure)
            .on_heartbeat(_on_heartbeat)
            .on_watchdog(_on_watchdog)
        )
    )


async def _deny_forced_push(event: Any, ctx: Any) -> Verdict:
    return Verdict.drop(Errno.EPOLICY, "forced push denied by policy")


async def main() -> None:
    import platform

    plugin = hook_matrix_full_plugin()
    builder = (
        kelir.builder(
            vcpus=2,
            memory_mb=2048,
            kernel=f"./fixtures/vmlinux-6.6-{platform.machine()}-kelir",
            rootfs_erofs="./fixtures/rootfs-hooks.erofs",
            tsc_locked=True,
            enable_watchdog=True,
            driver="auto",
        )
        .use(plugin)
        # see: cans/02-plugin-model.md#TrapFilter-predicates
        .trap(
            TrapFilter.binary("git")
            .with_arg("push")
            .with_cwd("/workspace/**")
            .with_env("CI", "1")
            .with_uid(0)
            .argv_matches(r"^--force$"),
            pre=_deny_forced_push,
        )
        # see: cans/02-plugin-model.md#Chain-decisions
        # on_trap_deny moves into .deny(|d| d.on_trap_deny(…)); the
        # verdict parameter is the DENYING Action.
        # see: cans/02-plugin-model.md#Hook-context-restriction
        # on_trap_deny(event, verdict, session): the verdict parameter is the
        # DENYING Action (the unified enum), so break-glass can say "denied by
        # X for Y, quarantine" — see: 03-sdk-architecture.md#Session-surfaces.
        # see: cans/10-attestation.md#Hook-trace
        # hook_trace read: every hook that fired for the trapped process
        # carries a HookSpan (plugin, ring, verdict_kind, duration_ns,
        # ordinal); diagnostic only, excluded from Merkle roots. The
        # comment-pinned HookSpan case (the hook-trace-coverage vector this
        # example stands in for; vectors.json cannot express a hook-trace
        # expectation today) asserts: { plugin: "hook-matrix-full",
        # ring: Trap, verdict_kind: Pass, duration_ns: 0, ordinal: 0 }.
        .deny(
            lambda d: d.on_trap_deny(
                lambda event, verdict, session: None,
            )
        )
        .seal()
    )

    async with builder as session:
        print(f"[kelir] booted driver={session.driver_name} "
              f"boot_ms={session.boot_ms}")

        probe = await session.spawn(
            "cat", [IDENTITY_PATH], cwd="/workspace",
        )
        print(f"[kelir] cat identity -> exit={probe.exit_code}")

        rm_probe = await session.spawn("rm", ["/workspace/test.trashable"])
        assert rm_probe.exit_code == 0
        print(f"[kelir] rm probe -> fs_unlinks={STATE.fs_unlinks} "
              f"fs_renames={STATE.fs_renames}")

        await asyncio.sleep(0.1)
        s = STATE
        print(f"[kelir] heartbeats={s.heartbeats} stalls={s.stalls} "
              f"exits={s.exits} failures={s.failures} traps={s.traps} "
              f"fs_locks={s.fs_locks} net_inbound={s.net_inbound} "
              f"exec_stdout={s.exec_stdout} exec_stderr={s.exec_stderr} "
              f"host_preempts={s.host_preempts} "
              f"vm_shutdowns={s.vm_shutdowns} "
              f"memory_pressures={s.memory_pressures}")


if __name__ == "__main__":
    asyncio.run(main())
