# see: cans/03-sdk-architecture.md#Python-SDK
# see: cans/02-plugin-model.md#Foreign-plugin-proxy
from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import TYPE_CHECKING, Any, Callable, Optional

from kelir import FilterAction, HandlerContext, Verdict
from kelir.plugin_fusion import HookDescriptor, HookStage
from kelir.generated.fusion_tables import HOOK_RINGS

if TYPE_CHECKING:
    from kelir.plugin_builder import PluginBuilder

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
        self._watchdog: tuple[Any, str] | None = None

    def set_watchdog(self, timeout: Any, action: str) -> None:
        # TODO (L3): arm the VM watchdog via the PyO3 _kelir bridge.
        self._watchdog = (timeout, action)

    def register_veto(
        self, event: VetoEvent, predicate: Callable[[], bool]
    ) -> None:
        # TODO (L3): register the veto predicate with the lifecycle
        self._vetoes.append((event, predicate))



# see: cans/02-plugin-model.md#Foreign-plugin-proxy
# see: cans/02-plugin-model.md#Hook-context-restriction
def hook_descriptors(plugin: object) -> list[HookDescriptor]:
    descriptors: list[HookDescriptor] = []
    if isinstance(plugin, BasePlugin):
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
    from kelir.plugin_builder import _PluginWrapper

    if isinstance(plugin, _PluginWrapper):
        return plugin.hook_descriptors()
    return descriptors


class BasePlugin:

    name: str = ""
    requires: tuple[str, ...] = ()

    # see: cans/02-plugin-model.md#KelirPlugin-trait
    def to_builder(self) -> PluginBuilder:
        from kelir.plugin_builder import PluginBuilder

        builder = PluginBuilder(self.name or type(self).__name__)
        for required in self.requires:
            builder.require(required)
        for method_name in HOOK_RINGS:
            overridden = getattr(type(self), method_name, None)
            base = getattr(BasePlugin, method_name, None)
            if overridden is None or overridden is base:
                continue
            getattr(builder, method_name)(getattr(self, method_name))
        return builder

    async def on_trap(self, event: Any) -> Optional[Verdict]:
        # TODO (L3): dispatch trap event to the plugin
        return None

    def on_fs_lookup(self, path: Path) -> Verdict:
        # TODO (L3): dispatch fs lookup to the plugin
        return Verdict.pass_()

    def on_fs_read(self, path: Path, buffer: bytes) -> bytes:
        # TODO (L3): dispatch fs read to the plugin
        return buffer

    def on_fs_write(
        self, path: Path, data: Any, pid: int, ctx: HandlerContext
    ) -> FilterAction:
        # TODO (L3): dispatch fs write to the plugin
        return FilterAction.forward()

    def on_net_outbound(self, frame: bytearray) -> FilterAction:
        # TODO (L3): dispatch outbound frame to the plugin
        return FilterAction.forward()

    async def on_console_input(self, port: int, data: bytes) -> bytes:
        # TODO (L3): dispatch console input to the plugin
        return data

    async def on_console_output(self, port: int, chunk: bytes) -> bytes:
        # TODO (L3): dispatch console output to the plugin
        return chunk

    # see: cans/02-plugin-model.md#KelirPlugin-trait
    async def on_exec_stdout(self, pid: int, exec_id: int, chunk: bytes) -> bytes:
        # TODO (L3): dispatch EXEC stream stdout to the plugin
        return chunk

    # see: cans/02-plugin-model.md#KelirPlugin-trait
    async def on_exec_stderr(self, pid: int, exec_id: int, chunk: bytes) -> bytes:
        # TODO (L3): dispatch EXEC stream stderr to the plugin
        return chunk

    async def on_heartbeat(self, event: Any) -> None:
        # TODO (L3): dispatch heartbeat telemetry to the plugin
        pass

    async def on_process_exit(self, event: Any) -> None:
        # TODO (L3): dispatch process-exit event to the plugin
        pass

    async def on_vm_init(self, ctx: Any) -> None:
        # TODO (L3): dispatch vm-init lifecycle event to the plugin
        pass

    async def on_vm_ready(self, ctx: Any) -> None:
        # TODO (L3): dispatch vm-ready lifecycle event to the plugin
        pass

    async def on_vm_shutdown(self, ctx: Any) -> None:
        # TODO (L3): dispatch vm-shutdown lifecycle event to the plugin
        pass

    async def on_watchdog(self, event: WatchdogEvent) -> WatchdogVerdict:
        # TODO (L3): dispatch watchdog event to the plugin
        return WatchdogVerdict.ExecuteDefault
