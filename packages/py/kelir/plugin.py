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
    Fork = "fork"
    Suspend = "suspend"
    ClockSkew = "clock_skew"


@dataclass
class WatchdogEvent:
    timeout_ms: int = 0
    action: str = "reboot"
    fired_at_ns: int = 0
    last_heartbeat_ms: int = 0
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
        # TODO: register the veto predicate with the lifecycle
        self._vetoes.append((event, predicate))


class BasePlugin:

    name: str = ""
    requires: tuple[str, ...] = ()

    async def on_trap(self, event: Any) -> Optional[Verdict]:
        # TODO: dispatch trap event to the plugin
        return None

    async def on_fs_lookup(self, path: Path) -> Verdict:
        # TODO: dispatch fs lookup to the plugin
        return Verdict.pass_()

    async def on_fs_read(self, path: Path, buffer: bytes) -> bytes:
        # TODO: dispatch fs read to the plugin
        return buffer

    async def on_fs_write(
        self, path: Path, data: Any, pid: int, ctx: HandlerContext
    ) -> FilterAction:
        # TODO: dispatch fs write to the plugin
        return FilterAction.forward()

    async def on_net_outbound(self, frame: bytearray) -> FilterAction:
        # TODO: dispatch outbound frame to the plugin
        return FilterAction.forward()

    async def on_console_input(self, port: int, data: bytes) -> bytes:
        # TODO: dispatch console input to the plugin
        return data

    async def on_console_output(self, port: int, chunk: bytes) -> bytes:
        # TODO: dispatch console output to the plugin
        return chunk

    async def on_heartbeat(self, event: Any) -> None:
        # TODO: dispatch heartbeat telemetry to the plugin
        pass

    async def on_process_exit(self, event: Any) -> None:
        # TODO: dispatch process-exit event to the plugin
        pass

    async def on_vm_init(self, ctx: Any) -> None:
        # TODO: dispatch vm-init lifecycle event to the plugin
        pass

    async def on_vm_ready(self, ctx: Any) -> None:
        # TODO: dispatch vm-ready lifecycle event to the plugin
        pass

    async def on_vm_shutdown(self, ctx: Any) -> None:
        # TODO: dispatch vm-shutdown lifecycle event to the plugin
        pass

    async def on_watchdog(self, event: WatchdogEvent) -> WatchdogVerdict:
        # TODO: dispatch watchdog event to the plugin
        return WatchdogVerdict.ExecuteDefault
