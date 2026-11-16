# see: cans/03-sdk-architecture.md#Python-SDK
# see: cans/02-plugin-model.md#Foreign-plugin-proxy
from __future__ import annotations

from dataclasses import dataclass
from datetime import timedelta
from enum import Enum
from pathlib import Path
from typing import TYPE_CHECKING, Any, Callable, ClassVar, Generic, Optional

from kelir import (
    Caps,
    CtxT,
    EConfig,
    FilterAction,
    HandlerContext,
    ParamsT,
    SpawnResult,
    TrapEvent,
    Verdict,
    VmHandle,
    WatchdogAction,
)
from kelir.plugin_fusion import HookDescriptor, HookStage
from kelir.generated.fusion_tables import HOOK_RINGS
from kelir.lifecycle_session import Session

if TYPE_CHECKING:
    from kelir import KelirBuilder

__all__ = [
    "BasePlugin",
    "FsInodeAttr",
    "HandlerContext",
    "LifecycleCtx",
    "PluginError",
    "Veto",
    "VetoEvent",
    "WatchdogEvent",
    "WatchdogVerdict",
    "plugin_error_to_filter_action",
    "plugin_error_to_verdict",
]


class PluginError(Exception):

    def __init__(self, errno: str = "", reason: str = "") -> None:
        super().__init__(f"{errno}: {reason}" if reason else errno)
        self.errno = errno
        self.reason = reason



    @classmethod
    def config(cls, message: str = "") -> "PluginError":
        # see: cans/03-sdk-architecture.md#Prerequisite-gating-across-languages
        return cls(errno="EConfig", reason=message)

    @classmethod
    def halt(cls, message: str = "") -> "PluginError":
        # see: cans/02-plugin-model.md#PluginError-scope-pin
        errno, _, reason = message.partition(": ")
        return cls(errno=errno, reason=reason) if reason else cls(errno=message)


# see: cans/02-plugin-model.md#Full-Verdict-lattice
def plugin_error_to_verdict(err: PluginError) -> Verdict:
    return Verdict.drop(err.errno, err.reason)


# see: cans/02-plugin-model.md#Full-Verdict-lattice
def plugin_error_to_filter_action(err: PluginError) -> FilterAction:
    return Verdict.drop(err.errno, err.reason)


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


# see: cans/08-lifecycle.md#Lifecycle-vetoes
@dataclass
class Veto:
    allowed: bool = False
    by: str = ""
    reason: str = ""
    recovery: str | None = None


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

    def __init__(self, caps: Caps | None = None) -> None:
        self._vetoes: list[tuple[VetoEvent, Callable[[], Veto]]] = []
        self._watchdog: tuple[timedelta, WatchdogAction] | None = None
        # see: cans/03-sdk-architecture.md#Service-lookup-across-languages
        self.cap = caps if caps is not None else Caps()
        self.vm = VmHandle(Session())

    # see: cans/03-sdk-architecture.md#Session-surfaces
    async def exec_in_guest(self, argv: list[str]) -> SpawnResult:
        # TODO (L3) [protocol_mux]: exec_in_guest rides the EXEC stream kind via the shim wire protocol.; slop: exec_in_guest always raises, no guest command path exists.
        raise EConfig(
            "kelir native PyO3 bridge (_kelir) is missing: lifecycle exec_in_guest (L3)"
        )

    # see: cans/08-lifecycle.md#Watchdog
    def set_watchdog(self, *, timeout: timedelta, action: WatchdogAction) -> None:
        # L3-GAP: [lc_18_watchdog_heartbeat_timeout_reboot]: arm the VM watchdog via the PyO3 _kelir bridge.; slop: watchdog never armed, deadlocked guest never reboots
        self._watchdog = (timeout, action)

    def register_veto(
        self,
        event: VetoEvent,
        predicate: Callable[[], Veto | bool],
    ) -> None:
        # see: cans/08-lifecycle.md#Lifecycle-vetoes
        def desugar() -> Veto:
            result = predicate()
            if isinstance(result, Veto):
                return result
            return Veto(allowed=result, by="", reason="", recovery=None)

        # L3-GAP: [plugin_engine]: register the veto predicate with the lifecycle; slop: veto predicate never registered, unsafe fork proceeds
        self._vetoes.append((event, desugar))



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
    from kelir import KelirBuilder

    if isinstance(plugin, KelirBuilder):
        return plugin.hook_descriptors()
    return descriptors


class BasePlugin(Generic[CtxT, ParamsT]):

    name: ClassVar[str] = ""
    requires: ClassVar[tuple[str, ...]] = ()

    # see: cans/02-plugin-model.md#KelirPlugin-trait
    def to_builder(self) -> KelirBuilder:
        from kelir import KelirBuilder

        builder = KelirBuilder.plugin(self.name or type(self).__name__)
        for required in self.requires:
            builder.require(required)
        for method_name in HOOK_RINGS:
            overridden = getattr(type(self), method_name, None)
            base = getattr(BasePlugin, method_name, None)
            if overridden is None or overridden is base:
                continue
            getattr(builder, method_name)(getattr(self, method_name))
        return builder

    async def on_trap(self, event: TrapEvent) -> Optional[Verdict]:
        # L3-GAP: [plugin_engine]: dispatch trap event to the plugin; slop: trap event never dispatched, plugin sees nothing
        return None

    def on_fs_lookup(self, path: Path) -> Verdict:
        # L3-GAP: [storage_plane]: dispatch fs lookup to the plugin; slop: fs lookup never dispatched, plugin filter bypassed
        return Verdict.pass_()

    def on_fs_read(self, path: Path, buffer: bytes) -> bytes:
        # L3-GAP: [storage_plane]: dispatch fs read to the plugin; slop: fs read never dispatched, masking never applies
        return buffer

    def on_fs_write(
        self, path: Path, data: bytes, pid: int, ctx: HandlerContext[CtxT, ParamsT]
    ) -> FilterAction:
        # L3-GAP: [storage_plane]: dispatch fs write to the plugin; slop: fs write never dispatched, quota never enforced
        return FilterAction.forward()

    def on_net_outbound(self, frame: bytearray) -> FilterAction:
        # L3-GAP: [wire_plane]: dispatch outbound frame to the plugin; slop: outbound frame never dispatched, egress unfiltered
        return FilterAction.forward()

    async def on_console_input(self, port: int, data: bytes) -> bytes:
        # L3-GAP: [terminal_plane]: dispatch console input to the plugin; slop: console input never dispatched, shield never sees keystrokes
        return data

    async def on_console_output(self, port: int, chunk: bytes) -> bytes:
        # L3-GAP: [terminal_plane]: dispatch console output to the plugin; slop: console output never dispatched, sanitizer never runs
        return chunk

    # see: cans/02-plugin-model.md#KelirPlugin-trait
    async def on_exec_stdout(self, pid: int, exec_id: int, chunk: bytes) -> bytes:
        # L3-GAP: [tp_09_exec_stream_sublane_stdio_fidelity]: dispatch EXEC stream stdout to the plugin; slop: EXEC stdout never dispatched, stdio fidelity unobserved
        return chunk

    # see: cans/02-plugin-model.md#KelirPlugin-trait
    async def on_exec_stderr(self, pid: int, exec_id: int, chunk: bytes) -> bytes:
        # L3-GAP: [tp_09_exec_stream_sublane_stdio_fidelity]: dispatch EXEC stream stderr to the plugin; slop: EXEC stderr never dispatched, stdio fidelity unobserved
        return chunk

    async def on_heartbeat(self, event: Any) -> None:
        # L3-GAP: [telemetry_audit]: dispatch heartbeat telemetry to the plugin; slop: heartbeat telemetry never dispatched, export gap hidden
        pass

    async def on_process_exit(self, event: Any) -> None:
        # L3-GAP: [attestation_receipts]: dispatch process-exit event to the plugin; slop: process-exit never dispatched, receipt never emitted
        pass

    async def on_vm_init(self, ctx: LifecycleCtx | None) -> None:
        # L3-GAP: [lifecycle_checkpoint]: dispatch vm-init lifecycle event to the plugin; slop: vm-init never dispatched, init hooks never run
        pass

    async def on_vm_ready(self, ctx: LifecycleCtx | None) -> None:
        # L3-GAP: [lifecycle_checkpoint]: dispatch vm-ready lifecycle event to the plugin; slop: vm-ready never dispatched, ready handlers never run
        pass

    async def on_vm_shutdown(self, ctx: LifecycleCtx | None) -> None:
        # L3-GAP: [lifecycle_checkpoint]: dispatch vm-shutdown lifecycle event to the plugin; slop: vm-shutdown never dispatched, syncfs never flushed
        pass

    async def on_watchdog(self, event: WatchdogEvent) -> WatchdogVerdict:
        # L3-GAP: [lc_18_watchdog_heartbeat_timeout_reboot]: dispatch watchdog event to the plugin; slop: watchdog event never dispatched, reboot never fires
        return WatchdogVerdict.ExecuteDefault
