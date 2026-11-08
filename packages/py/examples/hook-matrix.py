
from __future__ import annotations

import asyncio
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Optional

import kelir
from kelir import Deadline, FilterAction, Verdict
from kelir.plugin import (
    BasePlugin,
    PluginError,
    FsInodeAttr,
    HandlerContext,
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


# see: cans/04-terminal-plane.md#Terminal-pipeline
def safe_console_emit_len(window: bytes) -> int:
    emit = max(0, len(window) - CARRYOVER)
    return _back_off_incomplete_ansi(
        window, _back_off_incomplete_utf8(window, emit)
    )


def _back_off_incomplete_utf8(window: bytes, emit: int) -> int:
    if emit == 0 or emit >= len(window):
        return emit
    start = emit
    continuations = 0
    while start > 0 and continuations < 3 and 0x80 <= window[start - 1] < 0xC0:
        start -= 1
        continuations += 1
    if start == 0:
        return emit
    lead = window[start - 1]
    if 0xC2 <= lead < 0xE0:
        needed = 2
    elif 0xE0 <= lead < 0xF0:
        needed = 3
    elif 0xF0 <= lead < 0xF5:
        needed = 4
    else:
        return emit
    return start - 1 if start - 1 + needed > emit else emit


def _back_off_incomplete_ansi(window: bytes, emit: int) -> int:
    if emit == 0:
        return emit
    escape = -1
    for index in range(emit):
        if window[index] == 0x1B:
            escape = index
    if escape == -1:
        return emit
    if escape + 1 >= emit:
        return escape
    if window[escape + 1] == 0x5B:
        for byte in window[escape + 2:emit]:
            if 0x40 <= byte < 0x7F:
                return emit
        return escape
    if window[escape + 1] == 0x5D:
        index = escape + 2
        while index < emit:
            if window[index] == 0x07:
                return emit
            if window[index] == 0x1B and window[index + 1:index + 2] == b"\\":
                return emit
            index += 1
        return escape
    return emit


@dataclass
class HookState:
    tail: bytes = b""
    console_tail: bytes = b""
    heartbeats: int = 0
    last_uptime_ms: Optional[int] = None
    stalls: int = 0
    exits: int = 0
    failures: int = 0


class HookMatrixPlugin(BasePlugin):
    name = "hook-matrix"

    def __init__(self) -> None:
        self._state = HookState()

    @property
    def state(self) -> HookState:
        return self._state

    def on_net_outbound(self, frame: bytearray) -> FilterAction:
        if DENY_MARKER in frame:
            return FilterAction.suspend(until=Deadline(0))
        return FilterAction.forward()

    def on_fs_lookup(self, path: Path) -> Verdict:
        if escapes_workspace(str(path)):
            raise PluginError.reject(
                errno="E_PATH_TRAVERSAL",
                reason="lookup escapes the /workspace root",
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

    def on_fs_read(self, path: Path, buffer: bytes) -> bytes:
        window = self._state.tail + buffer
        masked = mask_tokens(window)
        chunk = masked[len(masked) - len(buffer):] if buffer else b""
        self._state.tail = masked[-CARRYOVER:] if masked else b""
        return chunk

    def on_fs_write(
        self, path: Path, data: bytearray, pid: int, ctx: HandlerContext,
    ) -> FilterAction:
        return FilterAction.forward()

    async def on_console_input(self, port: int, data: bytes) -> bytes:
        return _OSC52_RE.sub(b"", data)

    async def on_console_output(self, port: int, chunk: bytes) -> bytes:
        window = self._state.console_tail + chunk
        masked = mask_tokens(window)
        emit = safe_console_emit_len(masked)
        self._state.console_tail = masked[emit:]
        return masked[:emit]

    async def on_heartbeat(self, event: Any) -> None:
        if (self._state.last_uptime_ms == event.uptime_ms
                and self._state.heartbeats > 0):
            self._state.stalls += 1
        self._state.last_uptime_ms = event.uptime_ms
        self._state.heartbeats += 1

    async def on_process_exit(self, event: Any) -> None:
        self._state.exits += 1
        if event.exit_code != 0:
            self._state.failures += 1


async def main() -> None:
    import platform

    plugin = HookMatrixPlugin()
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
        .plugin(plugin)
    )

    async with builder as session:
        print(f"[kelir] booted driver={session.driver_name} "
              f"boot_ms={session.boot_ms}")

        probe = await session.spawn(
            "cat", [IDENTITY_PATH], cwd="/workspace",
        )
        print(f"[kelir] cat identity -> exit={probe.exit_code}")

        await asyncio.sleep(0.1)
        s = plugin.state
        print(f"[kelir] heartbeats={s.heartbeats} stalls={s.stalls} "
              f"exits={s.exits} failures={s.failures}")


if __name__ == "__main__":
    asyncio.run(main())
