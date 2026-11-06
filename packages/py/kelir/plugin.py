# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import Any, Callable, Optional

from kelir import FilterAction, HandlerContext, Verdict

__all__ = [
    "BasePlugin",
    "FsInodeAttr",
    "HandlerContext",
    "LifecycleCtx",
    "PluginError",
    "VetoEvent",
    "WatchdogEvent",
    "WatchdogVerdict",
]


class PluginError(Exception):

    def __init__(self, errno: str = "", reason: str = "") -> None:
        super().__init__(f"{errno}: {reason}" if reason else errno)
        self.errno = errno
        self.reason = reason

    @classmethod
    def reject(cls, errno: str = "", reason: str = "") -> PluginError:
        return cls(errno=errno, reason=reason)


@dataclass
class FsInodeAttr:
    mode: int = 0o400
    uid: int = 0
    gid: int = 0
    size: int = 0
    content: bytes = b""


class VetoEvent(Enum):
    Suspend = "suspend"
    ClockSkew = "clock_skew"


@dataclass
class WatchdogEvent:
    last_active_pid: Optional[int] = None


class WatchdogVerdict(Enum):
    Mitigate = "mitigate"
    ExecuteDefault = "execute_default"
    Suppress = "suppress"


class LifecycleCtx:

    def __init__(self) -> None:
        self._vetoes: list[tuple[VetoEvent, Callable[[], bool]]] = []

    def register_veto(
        self, event: VetoEvent, predicate: Callable[[], bool]
    ) -> None:
        # see: cans/03-sdk-architecture.md#Python-SDK
        self._vetoes.append((event, predicate))


class BasePlugin:

    name: str = ""
    requires: tuple[str, ...] = ()

    async def on_trap(self, event: Any) -> Optional[Verdict]:
        # see: cans/03-sdk-architecture.md#Python-SDK
        return None

    async def on_fs_lookup(self, path: Path) -> Verdict:
        # see: cans/03-sdk-architecture.md#Python-SDK
        return Verdict.pass_()

    async def on_fs_read(self, path: Path, buffer: bytes) -> bytes:
        # see: cans/03-sdk-architecture.md#Python-SDK
        return buffer

    async def on_fs_write(
        self, path: Path, data: Any, pid: int, ctx: HandlerContext
    ) -> Verdict:
        # see: cans/03-sdk-architecture.md#Python-SDK
        return Verdict.pass_()

    async def on_net_outbound(self, frame: bytearray) -> FilterAction:
        # see: cans/03-sdk-architecture.md#Python-SDK
        return FilterAction.forward()

    async def on_console_input(self, port: int, data: bytes) -> bytes:
        # see: cans/03-sdk-architecture.md#Python-SDK
        return data

    async def on_console_output(self, port: int, chunk: bytes) -> bytes:
        # see: cans/03-sdk-architecture.md#Python-SDK
        return chunk

    async def on_heartbeat(self, event: Any) -> None:
        # see: cans/03-sdk-architecture.md#Python-SDK
        pass

    async def on_process_exit(self, event: Any) -> None:
        # see: cans/03-sdk-architecture.md#Python-SDK
        pass

    async def on_vm_init(self, ctx: Any) -> None:
        # see: cans/03-sdk-architecture.md#Python-SDK
        pass

    async def on_vm_ready(self, ctx: Any) -> None:
        # see: cans/03-sdk-architecture.md#Python-SDK
        pass

    async def on_vm_shutdown(self, ctx: Any) -> None:
        # see: cans/03-sdk-architecture.md#Python-SDK
        pass

    async def on_watchdog(self, event: WatchdogEvent) -> WatchdogVerdict:
        # see: cans/03-sdk-architecture.md#Python-SDK
        return WatchdogVerdict.ExecuteDefault
