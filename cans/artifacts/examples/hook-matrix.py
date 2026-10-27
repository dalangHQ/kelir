# examples/hook-matrix.py
"""
Canonical depth example: the custom hook surface, not the config
surface. Where the swe-agent family configures wire/vfs/pty behavior
(SinkholeConfig, MaskRule, TokenShield), this file implements the
hooks themselves in one plugin, identically in hook-matrix.rs and
hook-matrix.ts.

SAME HOOK SET in all three SDKs, in dispatch order:

  1. on_net_outbound -> SyncVerdict. Frames containing the
     documentation-range marker 198.51.100. yield to the host for
     inspection (YieldToHost); all other frames Allow.
  2. on_fs_lookup -> Some(FsInodeAttr) for exactly one path,
     /workspace/.policy/identity, with mode 0o400 (no permission
     fallback, per 02); None — fall through — everywhere else.
  3. on_fs_read -> sliding-window masking. A 31-byte tail is carried
     between reads, so a secret token split across two reads is still
     masked from the split point onward and no single read ever emits
     a complete unmasked token (the token is 32 bytes).
  4. on_console_input -> strips OSC52 clipboard sequences
     (ESC ] 52 ; … terminator) from guest-bound input.
  5. on_console_output -> masks sk_live_ tokens in the chunk before it
     reaches the host terminal.
  6. on_heartbeat -> counts heartbeats; a receipt whose uptime_ms has
     not advanced since the previous receipt increments the stall
     counter (observation only, per 07).
  7. on_process_exit -> records (pid, exit_code); non-zero exits
     increment the failure counter.

Masking is same-length (token bytes overwritten with "*"), because
the read/output hooks mutate fixed-size buffers in place and cannot
change a chunk's length.

GUARDS (not new dispatch rows; hook order 1-7 above is unchanged):
  - on_fs_lookup rejects a path whose `..` segments resolve outside
    the /workspace root with E_PATH_TRAVERSAL (05), before the
    identity check runs. Paths without `..` keep the old behavior:
    the identity inode, else fall through.
  - on_fs_write rejects any payload over the 64KB (65536-byte) FFI
    inspection ceiling with BufferCeilingExceeded. That is the runtime
    inspection ceiling, NOT the swe-agent policy diff ceiling, which
    rejects with ENOSPC.

GAPS (flagged, not silently invented):
  - Python plugin-hook surface: only on_trap / on_fs_write /
    on_disclosure appear in the swe-agent Python plugin. The
    on_net_outbound / on_fs_lookup / on_fs_read / on_console_input /
    on_console_output / on_heartbeat / on_process_exit methods below mirror
    the Rust trait names in the existing snake_case idiom.
  - SyncVerdict / FsInodeAttr spellings in Python: no SyncVerdict
    value and no FsInodeAttr construction appear in any Python
    example. SyncVerdict.allow() / SyncVerdict.yield_to_host(0) below
    follow the PostExecAction.proceed() / rollback_and_halt() static
    precedent in the swe-agent Python example; the import path follows
    the kelir.plugin precedent (PluginError, TrapAction).
  - YieldToHost's numeric argument: unspecified anywhere (see
    hook-matrix.rs header); 0 is an explicit placeholder.
  - Hook return-vs-mutate shape: Rust mutates buffers in place; the
    Python hooks below return the transformed bytes (the swe-agent
    Python on_disclosure returns bytes rather than mutating), which is
    the Python-idiom equivalent of the same transformation.
  - Error idiom: no BufferCeilingExceeded variant or E_PATH_TRAVERSAL
    raise is attested in any Python example; both guards below use
    PluginError.reject (the swe-agent Python on_fs_write precedent),
    carrying the error name as the errno, distinct from ENOSPC.

Requirements:
    kelir>=3.0
"""

from __future__ import annotations

import asyncio
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

import kelir
from kelir.plugin import (
    BasePlugin,
    PluginError,
    SyncVerdict,
    FsInodeAttr,
)


# Guest path served as a synthetic inode, and the secret-token shape the
# masking hooks redact. TEST-NET-2 (198.51.100.0/24) is documentation
# space, used here as a denylist marker, not a real peer address.
IDENTITY_PATH = "/workspace/.policy/identity"
IDENTITY_CONTENT = b"policy-matrix-identity\n"
DENY_MARKER = b"198.51.100."
TOKEN_PREFIX = b"sk_live_"
TOKEN_LEN = len(TOKEN_PREFIX) + 24
# Bytes carried between reads: one less than the token length, so any
# token split across a read boundary is reassembled in the window.
CARRYOVER = TOKEN_LEN - 1

_TOKEN_RE = re.compile(rb"sk_live_[A-Za-z0-9]{24}")
_OSC52_RE = re.compile(rb"\x1b\]52;[^\x07\x1b]*(?:\x07|\x1b\\)?")


def escapes_workspace(path: str) -> bool:
    """Lexical workspace escape check, `..` paths only: resolve `..`
    segments without touching the filesystem (no symlink resolution
    here; the reconciler's RESOLVE_BENEATH guard owns that) and report
    whether the resolved path left the /workspace root. Paths with no
    `..` segment are untouched by this guard."""
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
    """Overwrite every byte of a complete sk_live_ token with b"*".
    A token is only masked when all 32 bytes are visible in the window;
    partial prefixes at the window's end are left for the next read's
    tail, never emitted as a complete token."""
    return _TOKEN_RE.sub(lambda m: b"*" * len(m.group(0)), window)


@dataclass
class HookState:
    tail: bytes = b""
    heartbeats: int = 0
    last_uptime_ms: Optional[int] = None
    stalls: int = 0
    exits: int = 0
    failures: int = 0


# Taxonomy: urn:kelir:sys:hook-matrix; rings: virtio-net, virtio-fs,
# virtio-console, virtio-vsock (heartbeat/exit events arrive over vsock).
class HookMatrixPlugin(BasePlugin):
    name = "hook-matrix"

    def __init__(self) -> None:
        self._state = HookState()

    @property
    def state(self) -> HookState:
        return self._state

    # Hook 1: wire outbound. Any frame carrying the denylist marker
    # yields to host inspection instead of leaving the TAP.
    async def on_net_outbound(self, frame: bytes) -> SyncVerdict:
        if DENY_MARKER in frame:
            # GAP: YieldToHost's numeric argument is unspecified; 0 is
            # an explicit placeholder (see header).
            return SyncVerdict.yield_to_host(0)
        return SyncVerdict.allow()

    # Hook 2: storage lookup. Exactly one synthetic inode, mode 0o400
    # with no permission fallback (02); everything else falls through.
    async def on_fs_lookup(self, path: Path) -> Optional[FsInodeAttr]:
        # GUARD: traversal first — an escaping path is rejected, never
        # served and never silently fallen through to the host fs.
        if escapes_workspace(str(path)):
            raise PluginError.reject(
                errno="E_PATH_TRAVERSAL",
                reason="lookup escapes the /workspace root",
            )
        if str(path) != IDENTITY_PATH:
            return None
        return FsInodeAttr(
            mode=0o400,
            uid=0,
            gid=0,
            size=len(IDENTITY_CONTENT),
            content=IDENTITY_CONTENT,
        )

    # Hook 3: storage read, sliding-window masking. Window = carried
    # tail + this chunk; mask over the window, emit the chunk's
    # portion, carry the last CARRYOVER bytes forward. Invariant: a
    # token split across reads is masked from the split point onward in
    # the later chunk, and no single chunk ever contains a complete
    # unmasked token, because the earlier chunk held at most a 31-byte
    # fragment of it.
    async def on_fs_read(self, path: Path, buffer: bytes) -> bytes:
        window = self._state.tail + buffer
        masked = mask_tokens(window)
        chunk = masked[len(masked) - len(buffer):] if buffer else b""
        self._state.tail = masked[-CARRYOVER:] if masked else b""
        return chunk

    # GUARD (not a new dispatch row): the 64KB FFI inspection ceiling.
    # A payload over it is rejected with BufferCeilingExceeded — the
    # runtime inspection error, distinct from the swe-agent policy diff
    # ceiling, which rejects oversize diffs with ENOSPC. GAP: no
    # BufferCeilingExceeded variant is attested in any Python example;
    # the swe-agent Python on_fs_write raises PluginError.reject, so
    # the ceiling name is carried as the errno here (see header).
    async def on_fs_write(self, path: Path, data: bytes) -> bool:
        ffi_inspection_ceiling = 64 * 1024
        if len(data) > ffi_inspection_ceiling:
            raise PluginError.reject(
                errno="BufferCeilingExceeded",
                reason=f"payload {len(data)} bytes exceeds "
                       f"{ffi_inspection_ceiling}-byte FFI ceiling",
            )
        return True

    # Hook 4: PTY input. Strip OSC52 clipboard exfiltration sequences:
    # ESC ] 52 ; … terminated by BEL (0x07) or ST (ESC \).
    async def on_console_input(self, data: bytes) -> bytes:
        return _OSC52_RE.sub(b"", data)

    # Hook 5: PTY output. Single-chunk masking; the cross-chunk case is
    # the vfs read path's sliding window above, not the terminal path.
    async def on_console_output(self, chunk: bytes) -> bytes:
        return mask_tokens(chunk)

    # Hook 6: heartbeat. Observation only (07): count, and note stalls,
    # but never gate.
    async def on_heartbeat(self, event) -> None:
        if (self._state.last_uptime_ms == event.uptime_ms
                and self._state.heartbeats > 0):
            self._state.stalls += 1
        self._state.last_uptime_ms = event.uptime_ms
        self._state.heartbeats += 1

    # Hook 7: process exit. Record, and count non-zero exits as failures.
    async def on_process_exit(self, event) -> None:
        self._state.exits += 1
        if event.exit_code != 0:
            self._state.failures += 1


# ============================================================================
# BUILDER + MAIN — boot, exercise the hooks, report counters
# ============================================================================

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

        # Reads the synthetic inode (lookup hook) through the masking
        # read path, and produces process-exit + heartbeat traffic for
        # the trap hooks. PTY input/output hooks fire on the session's
        # terminal.
        probe = await session.spawn(
            "cat", [IDENTITY_PATH], cwd="/workspace",
        )
        print(f"[kelir] cat identity -> exit={probe.exit_code}")

        # Let at least one heartbeat receipt arrive before reporting.
        await asyncio.sleep(0.1)
        s = plugin.state
        print(f"[kelir] heartbeats={s.heartbeats} stalls={s.stalls} "
              f"exits={s.exits} failures={s.failures}")


if __name__ == "__main__":
    asyncio.run(main())
