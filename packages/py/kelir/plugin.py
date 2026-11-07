# see: cans/03-sdk-architecture.md#Python-SDK
# see: cans/02-plugin-model.md#Foreign-plugin-proxy
from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import Any, Callable, Optional

from kelir import FilterAction, HandlerContext, Verdict
from kelir.fusion import HookDescriptor, HookStage, LifecycleTag, RingEvent

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


HOOK_RINGS: dict[str, tuple[RingEvent, LifecycleTag | None]] = {
    "on_trap": (RingEvent.TRAP, None),
    "on_fs_lookup": (RingEvent.FS_LOOKUP, None),
    "on_fs_read": (RingEvent.FS_READ, None),
    "on_fs_write": (RingEvent.FS_WRITE, None),
    "on_net_outbound": (RingEvent.NET_OUTBOUND, None),
    "on_console_input": (RingEvent.CONSOLE_INPUT, None),
    "on_console_output": (RingEvent.CONSOLE_OUTPUT, None),
    "on_heartbeat": (RingEvent.HEARTBEAT, None),
    "on_process_exit": (RingEvent.PROCESS_EXIT, None),
    "on_vm_init": (RingEvent.LIFECYCLE, "vm-init"),
    "on_vm_ready": (RingEvent.LIFECYCLE, "vm-ready"),
    "on_vm_shutdown": (RingEvent.LIFECYCLE, "vm-shutdown"),
    "on_watchdog": (RingEvent.WATCHDOG, None),
}


# see: cans/02-plugin-model.md#Foreign-plugin-proxy
def hook_descriptors(plugin: object) -> list[HookDescriptor]:
    descriptors: list[HookDescriptor] = []
    if not isinstance(plugin, BasePlugin):
        return descriptors
    name = getattr(plugin, "name", "") or type(plugin).__name__
    ordinal = 0
    for method_name, (ring, lifecycle_tag) in HOOK_RINGS.items():
        overridden = getattr(type(plugin), method_name, None)
        base = getattr(BasePlugin, method_name, None)
        if overridden is None or overridden is base:
            continue
        descriptors.append(
            HookDescriptor(
                ring=ring,
                stage=HookStage.SINGLE,
                ordinal=ordinal,
                plugin_name=name,
                lifecycle_tag=lifecycle_tag,
                run=getattr(plugin, method_name),
            )
        )
        ordinal += 1
    return descriptors


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
