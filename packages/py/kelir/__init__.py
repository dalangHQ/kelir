# see: cans/03-sdk-architecture.md#Python-SDK
# see: cans/02-plugin-model.md#Foreign-plugin-proxy
from __future__ import annotations

import importlib
from dataclasses import dataclass, field
from datetime import timedelta
from pathlib import Path
from types import SimpleNamespace, TracebackType
from typing import TYPE_CHECKING, Any, Awaitable, Callable, Literal, Self

if TYPE_CHECKING:
    from kelir.model_plugin import BasePlugin
    from kelir.plugin_builder import _PluginWrapper

from kelir.generated.wire_protocol import Channel, ExecStream, MsgType, StreamKind
from kelir.generated.attest_telemetry import (
    CausalDelta as CausalDelta,
    ExecutionReceipt as ExecutionReceipt,
    ExitKind as ExitKind,
    GuestClock as GuestClock,
    PhaseTimings as PhaseTimings,
    ReceiptExtension as ReceiptExtension,
    SnapshotDescriptor as SnapshotDescriptor,
)
from kelir.sdk_errors import EConfig as EConfig
from kelir.lifecycle_sandbox import Sandbox as Sandbox
from kelir.plugin_builder import PluginBuilder as PluginBuilder
from kelir.plugin_fusion import (
    FusedTrampoline,
    HookDescriptor,
    HookStage,
    PluginRegistration,
    RingEvent,
    TrapFilterSnapshot,
)


def _load_native_bridge() -> Any:
    # see: cans/03-sdk-architecture.md#Python-SDK
    try:
        return importlib.import_module("kelir._kelir")
    except ImportError:
        pass
    try:
        return importlib.import_module("_kelir")
    except ImportError as exc:
        raise EConfig(
            "kelir native PyO3 extension (_kelir) is missing: install the precompiled wheel (L3)"
        ) from exc


# see: cans/03-sdk-architecture.md#Python-SDK
def _require_native(native: Any, what: str) -> Any:
    if native is None:
        raise EConfig(f"kelir native PyO3 bridge (_kelir) is missing: {what} (L3)")
    return native


# see: cans/09-snapshots.md#Archives-and-portability
def _snapshot_from_dict(record: dict[str, Any]) -> SnapshotDescriptor:
    clock = record.get("guest_clock")
    guest_clock = None
    if isinstance(clock, dict):
        guest_clock = GuestClock(
            absolute_wall_clock_ns=int(clock.get("absolute_wall_clock_ns", 0)),
            monotonic_offset_ns=int(clock.get("monotonic_offset_ns", 0)),
            timezone=str(clock.get("timezone", "")),
            boot_params=str(clock.get("boot_params", "")),
        )
    parent_digest = record.get("parent_digest")
    return SnapshotDescriptor(
        id=str(record.get("id", "")),
        dirty_bytes=int(record.get("dirty_bytes", 0)),
        created_at_ns=int(record.get("created_at_ns", 0)),
        parent_digest=str(parent_digest) if isinstance(parent_digest, str) else None,
        guest_clock=guest_clock,
    )


# see: cans/09-snapshots.md#Archives-and-portability
def _snapshot_to_dict(snapshot: SnapshotDescriptor) -> dict[str, Any]:
    clock = snapshot.guest_clock
    return {
        "id": snapshot.id,
        "dirty_bytes": float(snapshot.dirty_bytes),
        "created_at_ns": float(snapshot.created_at_ns),
        "parent_digest": snapshot.parent_digest,
        "guest_clock": None
        if clock is None
        else {
            "absolute_wall_clock_ns": float(clock.absolute_wall_clock_ns),
            "monotonic_offset_ns": float(clock.monotonic_offset_ns),
            "timezone": clock.timezone,
            "boot_params": clock.boot_params,
        },
    }


# see: cans/03-sdk-architecture.md#Python-SDK
def _bridge_safe_config(kind: str, kwargs: dict[str, Any]) -> dict[str, Any]:
    # TODO (L3): token shields, prompts, and snoop callbacks cross once the FFI callback bridge lands.
    known = {
        "terminal": {
            "cols", "rows", "drop_osc52", "strip_title",
            "token_shield", "prompts", "stream_engine",
        },
        "storage": {
            "idmap", "shared_caches", "lock_broker", "synthetic_files",
            "mask_read", "snoops", "encapsulation", "scan_engine",
            "carryover_bytes",
        },
        "wire": {
            "sinkhole", "ssh_agent_bridge", "vaults", "l4_proxies",
            "tap_filter",
        },
    }[kind]
    unknown = set(kwargs) - known
    if unknown:
        raise EConfig(
            f"unknown {kind} plane config keys: {', '.join(sorted(unknown))}"
        )
    bridged: dict[str, Any] = {}
    for key, value in kwargs.items():
        if key in ("token_shield", "prompts", "snoops"):
            continue
        if key == "sinkhole" and value is not None:
            bridged["sinkhole"] = {
                "allowlist": list(value.allowlist),
                "drop_all": bool(value.drop_all),
            }
        elif key == "vaults" and value is not None:
            bridged["vaults"] = [
                {
                    "name": vault.name,
                    "kind": str(vault.kind),
                    "audience": vault.audience,
                    "placeholder": vault.placeholder,
                }
                for vault in value
            ]
        elif key == "l4_proxies" and value is not None:
            bridged["l4_proxies"] = [
                {
                    "listen_port": proxy.listen_port,
                    "destination": proxy.destination,
                }
                for proxy in value
            ]
        else:
            bridged[key] = value
    return bridged


# see: cans/11-protocol.md#Wire-messages
class Approval:
    pass


# see: cans/11-protocol.md#Wire-messages
class Handshake:
    pass


@dataclass
class Deadline:

    value: int = 0


# see: cans/11-protocol.md#Wire-framing
class ExecveMutation:

    def __init__(
        self,
        binary: str | None = None,
        argv: list[str] | None = None,
        env: dict[str, str] | None = None,
        unset_env: list[str] | None = None,
    ) -> None:
        self.binary = binary
        self.argv = list(argv) if argv is not None else None
        self.env = dict(env) if env is not None else {}
        self.unset_env = list(unset_env) if unset_env is not None else []

    # see: cans/11-protocol.md#Wire-framing
    def to_wire(self) -> dict[str, Any]:
        return {
            "new_binary": self.binary,
            "new_argv": self.argv,
            "set_env": [[key, value] for key, value in self.env.items()],
            "unset_env": self.unset_env,
        }


class Verdict:

    def __init__(self, kind: str, **payload: Any) -> None:
        self.kind = kind
        self.payload = payload

    @classmethod
    def pass_(cls) -> Verdict:
        return cls("pass")

    # see: cans/03-sdk-architecture.md#Idiomatic-Surfaces-Law
    Pass = pass_

    @classmethod
    def drop(cls, errno: str = "", reason: str = "") -> Verdict:
        return cls("drop", errno=errno, reason=reason)

    @classmethod
    def patch(
        cls,
        *args: Any,
        binary: str | None = None,
        argv: list[str] | None = None,
        env: dict[str, str] | None = None,
        unset_env: list[str] | None = None,
        name: str | None = None,
        cbor: dict[str, Any] | None = None,
        attr: Any = None,
    ) -> Verdict:
        # TODO (L3): serialize to FsInodeAttr / Custom payload
        if args and attr is None:
            attr = args[0]
        mutation = None
        if binary is not None or argv is not None or env is not None or unset_env is not None:
            mutation = ExecveMutation(
                binary=binary, argv=argv, env=env, unset_env=unset_env
            )
        return cls(
            "patch",
            binary=binary,
            argv=argv,
            env=env,
            unset_env=unset_env,
            mutation=mutation,
            name=name,
            cbor=cbor,
            attr=attr,
        )

    @classmethod
    def suspend(cls, until: Any = None) -> Verdict:
        return cls("suspend", until=until)

    @classmethod
    def abort(cls, reason: str = "") -> Verdict:
        return cls("abort", reason=reason)


class PostExecAction:

    def __init__(self, kind: str) -> None:
        self.kind = kind

    @classmethod
    def proceed(cls) -> PostExecAction:
        return cls("proceed")

    @classmethod
    def rollback_and_halt(cls) -> PostExecAction:
        return cls("rollback_and_halt")


class FilterAction:

    def __init__(self, kind: str, **payload: Any) -> None:
        self.kind = kind
        self.payload = payload

    @classmethod
    def forward(cls) -> FilterAction:
        return cls("forward")

    @classmethod
    def drop(cls, errno: str = "", reason: str = "") -> FilterAction:
        return cls("drop", errno=errno, reason=reason)

    @classmethod
    def suspend(cls, until: Any = None) -> FilterAction:
        return cls("suspend", until=until)

    # see: cans/02-plugin-model.md#Unified-verdict
    @classmethod
    def abort(cls, reason: str = "") -> FilterAction:
        return cls("abort", reason=reason)



@dataclass
class TrapEvent:
    binary_path: str = ""
    argv: list[str] = field(default_factory=list)
    pid: int = 0
    ppid: int = 0
    cwd: str = ""
    binary_blake3: str = ""


class TrapFilter:

    def __init__(self, binary: str | None = None, args: tuple[str, ...] = ()) -> None:
        self.binary_name = binary
        self.args = args

    @classmethod
    def binary(cls, name: str) -> TrapFilter:
        return cls(binary=name)

    def with_arg(self, arg: str) -> TrapFilter:
        return TrapFilter(self.binary_name, (*self.args, arg))

    def snapshot(self) -> TrapFilterSnapshot:
        # see: cans/02-plugin-model.md#Filter-before-FFI-gate
        return TrapFilterSnapshot(binary=self.binary_name, args=self.args)


# see: cans/03-sdk-architecture.md#Idiomatic-Surfaces-Law
@dataclass
class SpawnResult:
    exit_code: int = 0
    stdout: str | None = None
    stderr: str | None = None


@dataclass
class CommitStats:
    files_changed: int = 0
    tombstones_resolved: int = 0


# see: cans/03-sdk-architecture.md#Service-lookup-across-languages
class Caps:
    pass


class ClockHandle:
    def __init__(self, native: Any = None) -> None:
        self._native = native

    # see: cans/08-lifecycle.md#Clock-skew
    async def skew(self, delta: timedelta) -> None:
        native = _require_native(self._native, "vm clock skew")
        if not isinstance(delta, timedelta):
            raise EConfig("clock skew expects a datetime.timedelta, not a number")
        native.clock_skew(delta.total_seconds())


class VmHandle:
    def __init__(self, session: Session, native: Any = None) -> None:
        self._session = session
        self._native = native
        self.clock = ClockHandle(native)

    async def checkpoint(self, name: str) -> SnapshotDescriptor:
        native = _require_native(self._native, "vm checkpoint")
        return _snapshot_from_dict(native.checkpoint(name))

    async def rollback(self, snapshot: SnapshotDescriptor) -> None:
        native = _require_native(self._native, "vm rollback")
        native.rollback(_snapshot_to_dict(snapshot))

    async def fork(self) -> Session:
        return await self._session.fork()

    async def suspend(self, path: str) -> None:
        native = _require_native(self._native, "vm suspend")
        native.suspend(path)

    async def resume(self, path: str) -> None:
        native = _require_native(self._native, "vm resume")
        native.resume(path)

    async def adjust_balloon(self, target_mb: int) -> None:
        native = _require_native(self._native, "vm adjust_balloon")
        native.adjust_balloon(target_mb)

    async def set_vcpus(self, count: int) -> None:
        native = _require_native(self._native, "vm set_vcpus")
        native.set_vcpus(count)

    async def map_shared_region(self, tag: str, size_bytes: int) -> bytes:
        # TODO (L3): map a zero-copy host memory segment via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: vm map_shared_region (L3)")

    async def import_snapshot(self, data: bytes) -> None:
        native = _require_native(self._native, "vm import_snapshot")
        native.import_snapshot(data)

    async def export_snapshot(self) -> bytes:
        native = _require_native(self._native, "vm export_snapshot")
        return bytes(native.export_snapshot())


class WireHandle:
    def __init__(self, native: Any = None) -> None:
        self._native = native

    async def forward_port(self, guest_port: int, host_port: int) -> int:
        native = _require_native(self._native, "wire forward_port")
        return int(native.forward_port(guest_port, host_port))


class StorageHandle:
    def __init__(self, native: Any = None) -> None:
        self._native = native

    async def reconcile(
        self,
        cwd: str | None = None,
        atomic_staging: bool = False,
        fsync: bool = False,
    ) -> CommitStats:
        native = _require_native(self._native, "storage reconcile")
        files_changed, tombstones_resolved = native.reconcile(
            cwd if cwd is not None else "", atomic_staging, fsync
        )
        return CommitStats(
            files_changed=int(files_changed),
            tombstones_resolved=int(tombstones_resolved),
        )

    async def discard(self) -> None:
        native = _require_native(self._native, "storage discard")
        native.discard()


class VsockChannel:
    async def write(self, data: bytes) -> None:
        # TODO (L3): write frame to vsock channel via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: vsock write (L3)")

    async def read(self) -> bytes:
        # TODO (L3): read frame from vsock channel via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: vsock read (L3)")


class VsockHandle:
    async def channel(self, kind: int) -> VsockChannel:
        # TODO (L3): open duplex byte stream on vsock kind via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: vsock channel (L3)")


class FsHandle:
    async def warm_cache(self, paths: list[str]) -> None:
        # TODO (L3): warm shared caches into the guest via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: fs warm_cache (L3)")


class TerminalHandle:
    def __init__(self, native: Any = None) -> None:
        self._native = native

    def notice(self, text: str) -> None:
        # TODO (L3): publish TerminalNotice to the event bus via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: terminal notice (L3)")

    async def resize(self, cols: int, rows: int) -> None:
        native = _require_native(self._native, "terminal resize")
        native.resize(cols, rows)

    async def export_screen_matrix(self) -> bytes:
        native = _require_native(self._native, "terminal export_screen_matrix")
        return bytes(native.export_screen_matrix())

    async def read_from(self, port: int, offset: int) -> tuple[bytes, int]:
        native = _require_native(self._native, "terminal read_from")
        data, next_offset = native.read_from(port, offset)
        return bytes(data), int(next_offset)


class CheckpointHandle:
    def __init__(self, native: Any = None) -> None:
        self._native = native

    async def create(self, label: str) -> SnapshotDescriptor:
        native = _require_native(self._native, "checkpoint create")
        return _snapshot_from_dict(native.create(label))


class HostHandle:
    def __init__(self, native: Any = None) -> None:
        self._native = native

    async def lsm_status(self) -> bool:
        native = _require_native(self._native, "host lsm_status")
        return bool(native.lsm_status())

    async def pressure_level(self) -> int:
        native = _require_native(self._native, "host pressure_level")
        return int(native.pressure_level())


class StepHandle:
    def last(self) -> SnapshotDescriptor:
        # TODO (L3): return the most recently marked checkpoint via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: step last (L3)")



class HandlerContext:
    def __init__(self, trap: TrapEvent | None = None, cap: Caps | None = None) -> None:
        self.trap: TrapEvent = trap if trap is not None else TrapEvent()
        self.vm = VmHandle(Session())
        self.fs = FsHandle()
        self.terminal = TerminalHandle()
        self.ctx: Any = None
        self.step = StepHandle()
        self.cap = cap if cap is not None else Caps()
        self.params: Any = None

    async def receipt(self) -> ExecutionReceipt:
        # TODO (L3): fetch the trap's execution receipt via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: trap receipt (L3)")

    # see: cans/02-plugin-model.md#Unified-verdict
    def pass_(self) -> Verdict:
        return Verdict.pass_()

    # see: cans/02-plugin-model.md#Unified-verdict
    def drop(self, errno: str = "", reason: str = "") -> Verdict:
        return Verdict.drop(errno, reason)

    # see: cans/02-plugin-model.md#Unified-verdict
    def patch(self, *args: Any, **kwargs: Any) -> Verdict:
        return Verdict.patch(*args, **kwargs)

    # see: cans/02-plugin-model.md#Unified-verdict
    def suspend(self, until: Any = None) -> Verdict:
        return Verdict.suspend(until)

    # see: cans/02-plugin-model.md#Unified-verdict
    def abort(self, reason: str = "") -> Verdict:
        return Verdict.abort(reason)


# see: cans/08-lifecycle.md#Detached-lifecycle
def relay_socket_path(session_id: str) -> str:
    bridge = _load_native_bridge()
    try:
        return str(bridge.relay_socket_path(session_id))
    except Exception as exc:
        raise EConfig(str(exc)) from exc


class Session:
    def __init__(self) -> None:
        self._native: Any = None
        self.driver_name: str = ""
        self.boot_ms: int = 0
        self.cap = Caps()
        self.vm = VmHandle(self)
        self.terminal = TerminalHandle()
        self.storage = StorageHandle()
        self.wire = WireHandle()
        self.checkpoint = CheckpointHandle()
        self.host = HostHandle()
        self.vsock = VsockHandle()

    # see: cans/03-sdk-architecture.md#Python-SDK
    @classmethod
    def from_native(cls, handle: Any) -> Session:
        session = cls()
        session._native = handle
        session.driver_name = str(handle.driver_name)
        session.boot_ms = int(handle.boot_ms)
        session.vm = VmHandle(session, handle.vm)
        session.terminal = TerminalHandle(handle.terminal)
        session.storage = StorageHandle(handle.storage)
        session.wire = WireHandle(handle.wire)
        session.checkpoint = CheckpointHandle(handle.checkpoint)
        session.host = HostHandle(handle.host)
        return session

    async def fork(self) -> Session:
        native = _require_native(self._native, "session fork")
        return Session.from_native(native.fork())

    async def rollback(self, snapshot: SnapshotDescriptor | None = None) -> None:
        if snapshot is not None:
            await self.vm.rollback(snapshot)
            return
        # TODO (L3): call native driver rollback via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: session rollback (L3)")

    async def stop(self) -> None:
        native = _require_native(self._native, "session stop")
        native.stop()

    async def terminate(self) -> None:
        native = _require_native(self._native, "session terminate")
        native.terminate()

    async def spawn(
        self,
        binary: str,
        argv: list[str] | None = None,
        *,
        cwd: str | None = None,
        params: Any = None,
    ) -> SpawnResult:
        # TODO (L3): spawn binary in the guest via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: session spawn (L3)")

    async def exec_in_guest(self, binary: str, argv: list[str]) -> SpawnResult:
        # TODO (L3): exec binary in the guest via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: session exec_in_guest (L3)")

    async def suspend(self) -> None:
        native = _require_native(self._native, "session suspend")
        native.suspend(str(self.session_dir() / "suspend.img"))

    async def resume(self, path: str) -> None:
        native = _require_native(self._native, "session resume")
        native.resume(path)

    # see: cans/03-sdk-architecture.md#Sandbox-high-level-facade
    def session_dir(self) -> Path:
        native = _require_native(self._native, "session session_dir")
        member = getattr(native, "session_dir", None)
        if member is None:
            raise EConfig(
                "kelir native PyO3 bridge (_kelir) is missing: session handle session_dir() (A11)"
            )
        return Path(str(member()))

    async def console(self) -> None:
        # TODO (L3): open the PTY console master via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: session console (L3)")

    # see: cans/08-lifecycle.md#Detached-lifecycle
    @classmethod
    async def attach(cls, session_id: str) -> Session:
        relay = relay_socket_path(session_id)
        _ = relay
        # TODO (L3): connect to relay.sock, verify the catalog lease is live, and rebuild the session from the runner-owned state.
        raise EConfig("TODO: Session.attach (L3)")

    async def get_causal_delta_from_parent(self) -> CausalDelta:
        # TODO (L3): fetch causal delta from parent session via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: session get_causal_delta_from_parent (L3)")



@dataclass
class VsockServiceEntry:
    port: int
    service: str
    schema: dict[str, Any]
    handler: Callable[[bytes], Awaitable[bytes]]


class KelirBuilder:
    def __init__(
        self,
        *,
        vcpus: int | None = None,
        memory_mb: int | None = None,
        page_size_kb: int | None = None,
        deterministic_clock: bool | None = None,
        pinned_clocksource: str | None = None,
        preemption_grace_ms: int | None = None,
        kernel: str | None = None,
        rootfs_erofs: str | None = None,
        tsc_locked: bool | None = None,
        enable_watchdog: bool | None = None,
        driver: str | None = None,
        detached: bool | None = None,
    ) -> None:
        self._plugins: list[object] = []
        self._plugin_registrations: list[PluginRegistration] = []
        self._hooks: list[HookDescriptor] = []
        # TODO (L3): the engine dispatch reads these entries once register_interceptors wiring lands per cans/02-plugin-model.md#Static-dispatch.
        self._vsock_services: list[VsockServiceEntry] = []
        self._next_ordinal: int = 0
        self._session: Session | None = None
        self._params_model: type | None = None
        self._config: dict[str, Any] = {
            "vcpus": vcpus,
            "memory_mb": memory_mb,
            "page_size_kb": page_size_kb,
            "deterministic_clock": deterministic_clock,
            "pinned_clocksource": pinned_clocksource,
            "preemption_grace_ms": preemption_grace_ms,
            "kernel": kernel,
            "rootfs_erofs": rootfs_erofs,
            "tsc_locked": tsc_locked,
            "enable_watchdog": enable_watchdog,
            "driver": driver,
            "detached": detached,
        }

    # see: cans/08-lifecycle.md#Detached-lifecycle
    def detached(self, detached: bool) -> Self:
        self._config["detached"] = detached
        return self

    def is_detached(self) -> bool:
        return bool(self._config.get("detached", False))

    # see: cans/04-terminal-plane.md#Terminal-pipeline
    def terminal(self, **kwargs: Any) -> Self:
        self._config["terminal"] = kwargs
        return self

    # see: cans/05-storage-plane.md#Workspace-commit
    def storage(self, **kwargs: Any) -> Self:
        self._config["storage"] = kwargs
        return self

    # see: cans/07-wire-plane.md#DNS-policy-filter
    def wire(self, **kwargs: Any) -> Self:
        self._config["wire"] = kwargs
        return self

    # see: cans/03-sdk-architecture.md#Python-SDK
    def params(self, model: type) -> Self:
        # TODO (L3): validated params models ride the native boot once the PyO3 binding awaits.
        self._params_model = model
        return self

    # see: cans/02-plugin-model.md#KelirPlugin-trait
    def use(self, plugin: BasePlugin | _PluginWrapper) -> Self:
        from kelir.model_plugin import hook_descriptors

        self._plugins.append(plugin)
        name = getattr(plugin, "name", None) or type(plugin).__name__
        enabled = bool(getattr(plugin, "enabled", True))
        self._plugin_registrations.append(
            PluginRegistration(name=str(name), enabled=enabled)
        )
        if enabled:
            self._hooks.extend(hook_descriptors(plugin))
        return self

    def plugin(self, plugin: BasePlugin | _PluginWrapper) -> Self:
        return self.use(plugin)

    def trap(
        self,
        pattern: str | TrapFilter,
        pre: Callable[..., Awaitable[Any]] | None = None,
        post: Callable[..., Awaitable[Any]] | None = None,
    ) -> Self:
        # see: cans/02-plugin-model.md#Foreign-plugin-proxy
        snapshot = (
            pattern.snapshot()
            if isinstance(pattern, TrapFilter)
            else TrapFilterSnapshot(pattern=pattern)
        )
        if pre is not None:
            self._hooks.append(
                HookDescriptor(
                    ring=RingEvent.TRAP,
                    stage=HookStage.PRE,
                    ordinal=self._next_ordinal,
                    filter=snapshot,
                    run=pre,
                )
            )
            self._next_ordinal += 1
        if post is not None:
            self._hooks.append(
                HookDescriptor(
                    ring=RingEvent.TRAP,
                    stage=HookStage.POST,
                    ordinal=self._next_ordinal,
                    filter=snapshot,
                    run=post,
                )
            )
            self._next_ordinal += 1
        return self

    def register_custom_action(
        self,
        name: str,
        schema: dict[str, Any],
        handler: Callable[..., Awaitable[Any]],
    ) -> Self:
        # see: cans/02-plugin-model.md#Foreign-plugin-proxy
        self._hooks.append(
            HookDescriptor(
                ring=RingEvent.CUSTOM_ACTION,
                stage=HookStage.SINGLE,
                ordinal=self._next_ordinal,
                custom_action_name=name,
                run=handler,
            )
        )
        self._next_ordinal += 1
        return self

    # see: cans/11-protocol.md#Control-channel
    def register_vsock_service(
        self,
        port: int,
        service: str,
        schema: dict[str, Any],
        handler: Callable[[bytes], Awaitable[bytes]],
    ) -> Self:
        if not isinstance(port, int) or isinstance(port, bool) or port < 0 or port > 0xFFFFFFFF:
            raise EConfig(f"vsock service port {port} outside u32")
        self._vsock_services.append(
            VsockServiceEntry(port=port, service=service, schema=schema, handler=handler)
        )
        return self

    # see: cans/11-protocol.md#Control-channel
    def register_stream_kind(
        self,
        kind: int,
        schema: dict[str, Any],
        handler: Callable[[bytes], Awaitable[bytes]],
    ) -> Self:
        # see: cans/02-plugin-model.md#Foreign-plugin-proxy
        self._hooks.append(
            HookDescriptor(
                ring=RingEvent.VSOCK_SERVICE,
                stage=HookStage.SINGLE,
                ordinal=self._next_ordinal,
                vsock_kind=kind,
                run=handler,
            )
        )
        self._next_ordinal += 1
        return self

    _LIFECYCLE_TAGS: dict[str, Literal["vm-init", "vm-ready", "vm-shutdown", "trap-deny", "pre-commit", "post-commit"]] = {
        "on_vm_init": "vm-init",
        "on_vm_ready": "vm-ready",
        "on_vm_shutdown": "vm-shutdown",
        "on_trap_deny": "trap-deny",
        "on_pre_commit": "pre-commit",
        "on_post_commit": "post-commit",
    }

    def _lifecycle_decorator(
        self, name: str
    ) -> Callable[[Callable[..., Awaitable[Any]]], Callable[..., Awaitable[Any]]]:
        tag = self._LIFECYCLE_TAGS.get(name)
        if tag is None:
            raise EConfig(f"unknown lifecycle hook {name}")

        def decorator(
            fn: Callable[..., Awaitable[Any]],
        ) -> Callable[..., Awaitable[Any]]:
            self._hooks.append(
                HookDescriptor(
                    ring=RingEvent.LIFECYCLE,
                    stage=HookStage.SINGLE,
                    ordinal=self._next_ordinal,
                    lifecycle_tag=tag,
                    run=fn,
                )
            )
            self._next_ordinal += 1
            return fn

        return decorator

    @property
    def on_vm_init(self) -> Callable[[Callable[..., Awaitable[Any]]], Callable[..., Awaitable[Any]]]:
        return self._lifecycle_decorator("on_vm_init")

    @property
    def on_vm_ready(self) -> Callable[[Callable[..., Awaitable[Any]]], Callable[..., Awaitable[Any]]]:
        return self._lifecycle_decorator("on_vm_ready")

    @property
    def on_vm_shutdown(self) -> Callable[[Callable[..., Awaitable[Any]]], Callable[..., Awaitable[Any]]]:
        return self._lifecycle_decorator("on_vm_shutdown")

    @property
    def on_trap_deny(self) -> Callable[[Callable[..., Awaitable[Any]]], Callable[..., Awaitable[Any]]]:
        return self._lifecycle_decorator("on_trap_deny")

    @property
    def on_pre_commit(self) -> Callable[[Callable[..., Awaitable[Any]]], Callable[..., Awaitable[Any]]]:
        return self._lifecycle_decorator("on_pre_commit")

    @property
    def on_post_commit(self) -> Callable[[Callable[..., Awaitable[Any]]], Callable[..., Awaitable[Any]]]:
        return self._lifecycle_decorator("on_post_commit")

    @property
    def vsock_services(self) -> list[VsockServiceEntry]:
        return self._vsock_services

    # see: cans/02-plugin-model.md#Foreign-plugin-proxy
    def fuse(self) -> FusedTrampoline:
        return FusedTrampoline.fuse(self._hooks)

    async def boot(self) -> Session:
        # see: cans/03-sdk-architecture.md#Python-SDK
        registered = {registration.name for registration in self._plugin_registrations}
        for plugin in self._plugins:
            name = getattr(plugin, "name", None) or type(plugin).__name__
            required_names = getattr(plugin, "requires", ())
            if isinstance(required_names, str):
                required_names = (required_names,)
            for required in required_names:
                if required not in registered:
                    raise EConfig(f"missing prerequisite: {required} required by {name}")
        descriptor = self.fuse().describe()
        bridge = _load_native_bridge()
        native = bridge.PyKelirBuilder(
            vcpus=self._config["vcpus"],
            memory_mb=self._config["memory_mb"],
            kernel=self._config["kernel"],
            rootfs_erofs=self._config["rootfs_erofs"],
            tsc_locked=self._config["tsc_locked"],
            enable_watchdog=self._config["enable_watchdog"],
            driver=self._config["driver"],
        )
        # TODO (L3): forward page_size_kb/deterministic_clock/pinned_clocksource/preemption_grace_ms once the PyO3 constructor accepts them.
        if "terminal" in self._config:
            # TODO (L3): token_shield and prompt callbacks stay Python-side until the FFI callback bridge lands.
            native.set_terminal_config(
                _bridge_safe_config("terminal", self._config["terminal"])
            )
        if "storage" in self._config:
            # TODO (L3): snoop on_write callbacks stay Python-side until the FFI callback bridge lands.
            native.set_storage_config(
                _bridge_safe_config("storage", self._config["storage"])
            )
        if "wire" in self._config:
            native.set_wire_config(_bridge_safe_config("wire", self._config["wire"]))
        for registration in self._plugin_registrations:
            native.use_plugin(registration.name, registration.enabled)
        native.fuse_hooks(descriptor)
        native_handle = native.boot()
        session = Session.from_native(native_handle)
        # see: cans/03-sdk-architecture.md#Service-lookup-across-languages
        get_plugin_context = getattr(native_handle, "get_plugin_context", None)
        if get_plugin_context is None:
            if self._plugin_registrations:
                raise EConfig(
                    "kelir native PyO3 bridge (_kelir) is missing: session handle get_plugin_context(name) (B13)"
                )
        else:
            for registration in self._plugin_registrations:
                if not registration.enabled:
                    continue
                record = get_plugin_context(registration.name)
                if not isinstance(record, dict):
                    raise EConfig(
                        f"plugin '{registration.name}' context is not a mapping; capability struct cannot be materialized (B13)"
                    )
                setattr(session.cap, registration.name, SimpleNamespace(**record))
        return session

    async def __aenter__(self) -> Session:
        self._session = await self.boot()
        return self._session

    async def __aexit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: TracebackType | None,
    ) -> None:
        if self._session is not None:
            await self._session.terminate()
            self._session = None


def kelir() -> KelirBuilder:
    return KelirBuilder()


def builder(
    *,
    vcpus: int | None = None,
    memory_mb: int | None = None,
    page_size_kb: int | None = None,
    deterministic_clock: bool | None = None,
    pinned_clocksource: str | None = None,
    preemption_grace_ms: int | None = None,
    kernel: str | None = None,
    rootfs_erofs: str | None = None,
    tsc_locked: bool | None = None,
    enable_watchdog: bool | None = None,
    driver: str | None = None,
    detached: bool | None = None,
) -> KelirBuilder:
    return KelirBuilder(
        vcpus=vcpus,
        memory_mb=memory_mb,
        page_size_kb=page_size_kb,
        deterministic_clock=deterministic_clock,
        pinned_clocksource=pinned_clocksource,
        preemption_grace_ms=preemption_grace_ms,
        kernel=kernel,
        rootfs_erofs=rootfs_erofs,
        tsc_locked=tsc_locked,
        enable_watchdog=enable_watchdog,
        driver=driver,
        detached=detached,
    )


__all__ = [
    "Approval",
    "CausalDelta",
    "Channel",
    "CheckpointHandle",
    "Deadline",
    "ExecStream",
    "ExecveMutation",
    "ExecutionReceipt",
    "ExitKind",
    "FilterAction",
    "GuestClock",
    "Handshake",
    "HandlerContext",
    "HostHandle",
    "KelirBuilder",
    "MsgType",
    "PhaseTimings",
    "PluginBuilder",
    "PostExecAction",
    "ReceiptExtension",
    "CommitStats",
    "Sandbox",
    "Session",
    "SnapshotDescriptor",
    "SpawnResult",
    "StreamKind",
    "TrapEvent",
    "TrapFilter",
    "Verdict",
    "builder",
    "kelir",
    "relay_socket_path",
]
