# see: cans/03-sdk-architecture.md#Python-SDK
# see: cans/02-plugin-model.md#Foreign-plugin-proxy
from __future__ import annotations

import importlib
import os
from dataclasses import dataclass, field
from pathlib import Path
from types import TracebackType
from typing import TYPE_CHECKING, Any, Awaitable, Callable, Literal, Self

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
from kelir.plugin_fusion import (
    FusedTrampoline,
    HookDescriptor,
    HookStage,
    PluginRegistration,
    RingEvent,
    TrapFilterSnapshot,
)

if TYPE_CHECKING:
    from pydantic import BaseModel


def _load_native_bridge() -> Any:
    # TODO (L3): bind the precompiled PyO3 _kelir extension once its boot entry point lands.
    try:
        return importlib.import_module("_kelir")
    except ImportError as exc:
        raise EConfig(
            "kelir native PyO3 extension (_kelir) is missing: install the precompiled wheel (L3)"
        ) from exc


# see: cans/11-protocol.md#Wire-messages
class Approval:
    pass


# see: cans/11-protocol.md#Wire-messages
class Handshake:
    pass


@dataclass
class Deadline:

    value: int = 0


class Verdict:

    def __init__(self, kind: str, **payload: Any) -> None:
        self.kind = kind
        self.payload = payload

    @classmethod
    def pass_(cls) -> Verdict:
        return cls("pass")

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
        name: str | None = None,
        cbor: dict[str, Any] | None = None,
        attr: Any = None,
    ) -> Verdict:
        # TODO (L3): serialize to ExecveMutation / FsInodeAttr / Custom payload
        if args and attr is None:
            attr = args[0]
        return cls(
            "patch",
            binary=binary,
            argv=argv,
            env=env,
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


@dataclass
class SpawnResult:
    exit_code: int = 0


@dataclass
class CommitStats:
    files_changed: int = 0
    tombstones_resolved: int = 0


@dataclass
class FinalReceipt:
    merkle_root: str = ""
    signed_jws: str = ""



class LedgerService:
    async def flush(self) -> None:
        # TODO (L3): flush host ledger via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: ledger flush (L3)")

    async def count_denials(self, ppid: int) -> int:
        # TODO (L3): count denials for ppid in host ledger via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: ledger count_denials (L3)")

    async def finalize(
        self,
        include_delta: CausalDelta | None = None,
        include_merkle_root: bool = False,
    ) -> FinalReceipt:
        # TODO (L3): finalize signed Merkle receipt via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: ledger finalize (L3)")

    async def get_causal_delta_from_parent(self) -> CausalDelta:
        # TODO (L3): fetch causal delta from parent session via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: ledger get_causal_delta_from_parent (L3)")

    async def blake3_of_producing_process(self, path: Any) -> str:
        # TODO (L3): resolve producing-process digest via ledger and the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: ledger blake3_of_producing_process (L3)")


class DbService:
    def __init__(self) -> None:
        self.rewrite_target: str = "migrate_schema."


class PolicyCap:
    def __init__(self) -> None:
        self.violations: int = 0


class TransactionScope:

    def __init__(
        self,
        name: str,
        service: TransactionService | None = None,
        paths: list[str] | None = None,
        schemas: list[str] | None = None,
    ) -> None:
        self.name = name
        self._service = service
        self.paths: list[str] = list(paths) if paths is not None else []
        self.schemas: list[str] = list(schemas) if schemas is not None else []
        self._handle: str | None = None

    async def __aenter__(self) -> TransactionScope:
        if self._service is None:
            # TODO (L3): begin 2PC scope via the PyO3 _kelir bridge.
            raise EConfig("kelir native PyO3 bridge (_kelir) is missing: transaction scope begin (L3)")
        self._handle = await self._service.begin_scope(
            self.name, self.paths, self.schemas
        )
        return self

    async def __aexit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: TracebackType | None,
    ) -> None:
        if self._handle is None:
            return None
        if self._service is None:
            # TODO (L3): abort the begun 2PC scope via the PyO3 _kelir bridge.
            raise EConfig("kelir native PyO3 bridge (_kelir) is missing: transaction scope abort (L3)")
        handle = self._handle
        self._handle = None
        if exc_type is None:
            await self._service.commit(handle)
        else:
            await self._service.abort(handle)
        return None

    async def savepoint(self) -> str:
        if self._handle is None or self._service is None:
            # TODO (L3): create savepoint via the PyO3 _kelir bridge.
            raise EConfig("kelir native PyO3 bridge (_kelir) is missing: transaction savepoint (L3)")
        return await self._service.savepoint(self._handle)

    async def rollback_to_savepoint(self, savepoint: str) -> None:
        if self._handle is None or self._service is None:
            # TODO (L3): roll back to savepoint via the PyO3 _kelir bridge.
            raise EConfig("kelir native PyO3 bridge (_kelir) is missing: transaction rollback_to_savepoint (L3)")
        await self._service.rollback_to_savepoint(self._handle, savepoint)


class TransactionService:
    async def begin_scope(
        self,
        name: str,
        paths: list[str] | None = None,
        schemas: list[str] | None = None,
    ) -> str:
        # TODO (L3): open a 2PC scope declaring the blast radius via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: transaction begin_scope (L3)")

    async def commit(self, handle: str) -> None:
        # TODO (L3): run _pre_commit (FUSE drain) then broker + SQL commit via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: transaction commit (L3)")

    async def abort(self, handle: str) -> None:
        # TODO (L3): run _post_commit(False) (unlink staging, drop broker upper) then SQL rollback via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: transaction abort (L3)")

    async def savepoint(self, handle: str) -> str:
        # TODO (L3): create savepoint via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: transaction savepoint (L3)")

    async def rollback_to_savepoint(self, handle: str, savepoint: str) -> None:
        # TODO (L3): roll back to savepoint via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: transaction rollback_to_savepoint (L3)")

    def scope(
        self,
        name: str,
        paths: list[str] | None = None,
        schemas: list[str] | None = None,
    ) -> TransactionScope:
        return TransactionScope(name, self, paths, schemas)


# see: cans/03-sdk-architecture.md#Service-lookup-across-languages
class Caps:

    def __init__(self) -> None:
        self.ledger = LedgerService()
        self.db = DbService()
        self.policy = PolicyCap()
        self.transaction = TransactionService()



class ClockHandle:
    async def skew(self, delta: Any) -> None:
        # TODO (L3): skew guest clock via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: vm clock skew (L3)")


class VmHandle:
    def __init__(self, session: Session) -> None:
        self._session = session
        self.clock = ClockHandle()

    async def checkpoint(self, name: str) -> SnapshotDescriptor:
        # TODO (L3): checkpoint VM state via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: vm checkpoint (L3)")

    async def rollback(self, snapshot: SnapshotDescriptor) -> None:
        # TODO (L3): roll back VM state to snapshot via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: vm rollback (L3)")

    async def fork(self) -> Session:
        # TODO (L3): CoW-fork the microVM via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: vm fork (L3)")

    async def suspend(self, path: str) -> None:
        # TODO (L3): suspend VM to snapshot file via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: vm suspend (L3)")

    async def resume(self, path: str) -> None:
        # TODO (L3): restore a suspended VM image via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: vm resume (L3)")

    async def adjust_balloon(self, target_mb: int) -> None:
        # TODO (L3): adjust the guest RAM ceiling via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: vm adjust_balloon (L3)")

    async def set_vcpus(self, count: int) -> None:
        # TODO (L3): park/unpark host vCPU threads via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: vm set_vcpus (L3)")

    async def map_shared_region(self, tag: str, size_bytes: int) -> bytes:
        # TODO (L3): map a zero-copy host memory segment via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: vm map_shared_region (L3)")

    async def import_snapshot(self, data: bytes) -> None:
        # TODO (L3): hydrate registers/dirty pages from snapshot bytes via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: vm import_snapshot (L3)")

    async def export_snapshot(self) -> bytes:
        # TODO (L3): export snapshot bytes via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: vm export_snapshot (L3)")


class WireHandle:
    async def forward_port(self, guest_port: int, host_port: int) -> int:
        # TODO (L3): forward guest port to host port via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: wire forward_port (L3)")


class StorageHandle:
    async def reconcile(
        self,
        cwd: str | None = None,
        atomic_staging: bool = False,
        fsync: bool = False,
    ) -> CommitStats:
        # TODO (L3): broker commit of the workspace via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: storage reconcile (L3)")

    async def discard(self) -> None:
        # TODO (L3): drop the sparse upper file and tombstones via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: storage discard (L3)")


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
    def notice(self, text: str) -> None:
        # TODO (L3): publish TerminalNotice to the event bus via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: terminal notice (L3)")

    async def resize(self, cols: int, rows: int) -> None:
        # TODO (L3): resize the guest terminal via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: terminal resize (L3)")

    async def export_screen_matrix(self) -> bytes:
        # TODO (L3): export the terminal screen matrix via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: terminal export_screen_matrix (L3)")

    async def read_from(self, port: int, offset: int) -> tuple[bytes, int]:
        # TODO (L3): read terminal replay bytes from port at offset via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: terminal read_from (L3)")


class CheckpointHandle:
    async def create(self, label: str) -> SnapshotDescriptor:
        # TODO (L3): create a checkpoint snapshot via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: checkpoint create (L3)")


class HostHandle:
    async def lsm_status(self) -> bool:
        # TODO (L3): read host LSM status via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: host lsm_status (L3)")

    async def pressure_level(self) -> int:
        # TODO (L3): read host memory-pressure level via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: host pressure_level (L3)")


class StepHandle:
    def last(self) -> SnapshotDescriptor:
        # TODO (L3): return the most recently marked checkpoint via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: step last (L3)")



class HandlerContext:
    def __init__(self, trap: TrapEvent | None = None) -> None:
        self.trap: TrapEvent = trap if trap is not None else TrapEvent()
        self.vm = VmHandle(Session())
        self.fs = FsHandle()
        self.terminal = TerminalHandle()
        self.ctx: Any = None
        self.step = StepHandle()
        self.cap = Caps()

    async def receipt(self) -> ExecutionReceipt:
        # TODO (L3): fetch the trap's execution receipt via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: trap receipt (L3)")



# see: cans/08-lifecycle.md#Detached-lifecycle
def relay_socket_path(session_id: str) -> str:
    if len(session_id) != 32 or any(
        c not in "0123456789abcdefABCDEF" for c in session_id
    ):
        raise EConfig(f"attach: session id `{session_id}` is not 32 hex chars")
    home = os.path.expanduser("~")
    if not home:
        raise EConfig("HOME is not set; cannot locate ~/.kelir relay socket")
    return str(
        Path(home) / ".kelir" / "sessions" / session_id.lower() / "relay.sock"
    )


class Session:
    def __init__(self) -> None:
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

    async def rollback(self, snapshot: SnapshotDescriptor | None = None) -> None:
        if snapshot is not None:
            await self.vm.rollback(snapshot)
            return
        # TODO (L3): call native driver rollback via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: session rollback (L3)")

    async def stop(self) -> None:
        # TODO (L3): stop the microVM session via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: session stop (L3)")

    async def terminate(self) -> None:
        # TODO (L3): terminate the microVM session via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: session terminate (L3)")

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

    async def suspend(self, path: str) -> None:
        # TODO (L3): suspend session to snapshot file via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: session suspend (L3)")

    async def resume(self, path: str) -> None:
        # TODO (L3): resume session from snapshot file via the PyO3 _kelir bridge.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: session resume (L3)")

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
        self._config: dict[str, Any] = {
            "vcpus": vcpus,
            "memory_mb": memory_mb,
            "kernel": kernel,
            "rootfs_erofs": rootfs_erofs,
            "tsc_locked": tsc_locked,
            "enable_watchdog": enable_watchdog,
            "driver": driver,
            "detached": detached,
        }
        self._params_model: type | None = None

    # see: cans/08-lifecycle.md#Detached-lifecycle
    def detached(self, detached: bool) -> Self:
        self._config["detached"] = detached
        return self

    def is_detached(self) -> bool:
        return bool(self._config.get("detached", False))

    def use(self, plugin: object) -> Self:
        # TODO (L2): validate prerequisites at boot
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

    def plugin(self, plugin: object) -> Self:
        return self.use(plugin)

    def terminal(self, **kwargs: Any) -> Self:
        # TODO (L3): configure the terminal plane
        self._config["terminal"] = kwargs
        return self

    def storage(self, **kwargs: Any) -> Self:
        # TODO (L3): configure the storage plane
        self._config["storage"] = kwargs
        return self

    def wire(self, **kwargs: Any) -> Self:
        # TODO (L3): configure the wire plane
        self._config["wire"] = kwargs
        return self

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

    def params(self, model: "type[BaseModel]") -> Self:
        self._params_model = model
        return self

    @property
    def vsock_services(self) -> list[VsockServiceEntry]:
        return self._vsock_services

    # see: cans/02-plugin-model.md#Foreign-plugin-proxy
    def fuse(self) -> FusedTrampoline:
        return FusedTrampoline.fuse(self._hooks)

    async def boot(self) -> Session:
        descriptor = self.fuse().describe()
        bridge = _load_native_bridge()
        native = bridge.PyKelirBuilder(
            vcpus=self._config["vcpus"],
            memory_mb=self._config["memory_mb"],
            kernel=self._config["kernel"],
            rootfs_erofs=self._config["rootfs_erofs"],
            tsc_locked=self._config["tsc_locked"],
            enable_watchdog=self._config["enable_watchdog"],
        )
        for registration in self._plugin_registrations:
            native.use_plugin(registration.name, registration.enabled)
        native.fuse_hooks(descriptor)
        # TODO (L3): await the native boot coroutine and wrap the PyO3 session handle into kelir.Session.
        raise EConfig("kelir native PyO3 boot binding is not implemented (L3)")

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
            await self._session.stop()
            self._session = None


def kelir() -> KelirBuilder:
    return KelirBuilder()


def builder(
    *,
    vcpus: int | None = None,
    memory_mb: int | None = None,
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
    "PostExecAction",
    "ReceiptExtension",
    "CommitStats",
    "FinalReceipt",
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
