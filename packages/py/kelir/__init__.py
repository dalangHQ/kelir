# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

from dataclasses import dataclass, field
from types import TracebackType
from typing import TYPE_CHECKING, Any, Awaitable, Callable, Self

if TYPE_CHECKING:
    from pydantic import BaseModel


class EConfig(Exception):
    pass



class Approval:
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
        # TODO: serialize to ExecveMutation / FsInodeAttr / Custom payload
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



@dataclass
class TrapEvent:
    binary_path: str = ""
    argv: list[str] = field(default_factory=list)
    pid: int = 0
    ppid: int = 0
    cwd: str = ""
    binary_blake3: bytes = b""


class TrapFilter:

    def __init__(self, binary: str | None = None, args: tuple[str, ...] = ()) -> None:
        self.binary_name = binary
        self.args = args

    @classmethod
    def binary(cls, name: str) -> TrapFilter:
        return cls(binary=name)

    def with_arg(self, arg: str) -> TrapFilter:
        return TrapFilter(self.binary_name, (*self.args, arg))


@dataclass
class ReceiptExtension:
    namespace: str = ""
    payload: bytes = b""


@dataclass
class ExecutionReceipt:
    binary_blake3: str = ""
    egress_bytes: int = 0
    dirty_bytes: int = 0
    extensions: list[ReceiptExtension] = field(default_factory=list)

    def __await__(self) -> Any:
        async def _return_self() -> ExecutionReceipt:
            return self

        return _return_self().__await__()


@dataclass
class SnapshotDescriptor:
    name: str = ""


@dataclass
class SpawnResult:
    exit_code: int = 0


@dataclass
class CausalDelta:
    pass


@dataclass
class ReconcileStats:
    files_changed: int = 0
    tombstones_resolved: int = 0


@dataclass
class FinalReceipt:
    merkle_root: str = ""
    signed_jws: str = ""



class LedgerService:
    async def flush(self) -> None:
        # TODO: flush host ledger
        pass

    async def count_denials(self, ppid: int) -> int:
        # TODO: count denials for ppid in host ledger
        return 0

    async def finalize(
        self,
        include_delta: CausalDelta | None = None,
        include_merkle_root: bool = False,
    ) -> FinalReceipt:
        # TODO: finalize signed Merkle receipt
        return FinalReceipt()

    async def get_causal_delta_from_parent(self) -> CausalDelta:
        # TODO: fetch causal delta from parent session
        return CausalDelta()

    async def blake3_of_producing_process(self, path: Any) -> str:
        # TODO: resolve producing-process digest via ledger
        return ""


class ContainerService:
    async def ingest_image(self, tarball: str) -> str:
        # TODO: ingest pre-cached OCI tarball, return image ref
        return tarball


class DbService:
    def __init__(self) -> None:
        self.rewrite_target: str = "migrate_schema."


class PolicyCap:
    def __init__(self) -> None:
        self.violations: int = 0


class TransactionScope:

    def __init__(self, name: str) -> None:
        self.name = name

    async def __aenter__(self) -> TransactionScope:
        # TODO: begin 2PC scope
        return self

    async def __aexit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: TracebackType | None,
    ) -> None:
        # TODO: commit on clean exit, abort on error
        pass

    async def savepoint(self) -> str:
        # TODO: create savepoint, return its handle
        return ""

    async def rollback_to_savepoint(self, savepoint: str) -> None:
        # TODO: roll back to savepoint
        pass


class TransactionService:
    def scope(
        self,
        name: str,
        paths: list[str] | None = None,
        schemas: list[str] | None = None,
    ) -> TransactionScope:
        # TODO: open a 2PC scope declaring blast radius
        return TransactionScope(name)


class Caps:

    def __init__(self) -> None:
        self.ledger = LedgerService()
        self.container = ContainerService()
        self.db = DbService()
        self.policy = PolicyCap()
        self.transaction = TransactionService()



class ClockHandle:
    async def skew(self, delta: Any) -> None:
        # TODO: skew guest clock
        pass


class VmHandle:
    def __init__(self, session: Session) -> None:
        self._session = session
        self.clock = ClockHandle()

    async def checkpoint(self, name: str) -> SnapshotDescriptor:
        # TODO: checkpoint VM state
        return SnapshotDescriptor(name=name)

    async def rollback(self, snapshot: SnapshotDescriptor) -> None:
        # TODO: roll back VM state to snapshot
        pass

    async def fork(self) -> Session:
        # TODO: CoW-fork the microVM
        return Session()

    async def suspend(self, path: str) -> None:
        # TODO: suspend VM to snapshot file
        pass

    async def import_snapshot(self, data: bytes) -> None:
        # TODO: hydrate registers/dirty pages from snapshot bytes
        pass

    async def export_snapshot(self) -> bytes:
        # TODO: export snapshot bytes
        return b""


class WireHandle:
    async def forward_port(self, guest_port: int, host_port: int) -> None:
        # TODO: forward guest port to host port
        pass


class StorageHandle:
    async def reconcile(
        self,
        cwd: str | None = None,
        atomic_staging: bool = False,
        fsync: bool = False,
    ) -> ReconcileStats:
        # TODO: broker commit of the workspace
        return ReconcileStats()


class VsockChannel:
    async def write(self, data: bytes) -> None:
        # TODO: write frame to vsock channel
        pass

    async def read(self) -> bytes:
        # TODO: read frame from vsock channel
        return b""


class VsockHandle:
    async def channel(self, kind: int) -> VsockChannel:
        # TODO: open duplex byte stream on vsock kind
        return VsockChannel()


class FsHandle:
    async def warm_cache(self, paths: list[str]) -> None:
        # TODO: warm shared caches into the guest
        pass


class TerminalHandle:
    def notice(self, text: str) -> None:
        # TODO: publish TerminalNotice to the event bus
        pass


class StepHandle:
    def last(self) -> SnapshotDescriptor:
        # TODO: return the most recently marked checkpoint
        return SnapshotDescriptor()



class HandlerContext:
    def __init__(self, trap: TrapEvent | None = None) -> None:
        self.trap: TrapEvent = trap if trap is not None else TrapEvent()
        self.vm = VmHandle(Session())
        self.fs = FsHandle()
        self.terminal = TerminalHandle()
        self.ctx: Any = None
        self.step = StepHandle()
        self.cap = Caps()

    def receipt(self) -> ExecutionReceipt:
        # TODO: fetch the trap's execution receipt (awaitable: see
        return ExecutionReceipt()



class VmmDriver:
    async def set_watchdog(self, timeout: Any = None, action: str = "") -> None:
        # TODO: arm the VM watchdog
        pass



class Session:
    def __init__(self) -> None:
        self.driver_name: str = ""
        self.boot_ms: int = 0
        self.cap = Caps()
        self.vm = VmHandle(self)
        self.wire = WireHandle()
        self.storage = StorageHandle()
        self.vsock = VsockHandle()

    async def rollback(self) -> None:
        # TODO: Call native driver rollback via PyO3 handle
        pass

    async def stop(self) -> None:
        pass

    async def terminate(self) -> None:
        # TODO: terminate the microVM session
        pass

    async def spawn(
        self,
        binary: str,
        argv: list[str] | None = None,
        *,
        cwd: str | None = None,
        params: Any = None,
    ) -> SpawnResult:
        # TODO: spawn binary in the guest (params cross as KELIR_PARAMS)
        return SpawnResult()

    async def exec_in_guest(self, binary: str, argv: list[str]) -> SpawnResult:
        # TODO: exec binary in the guest without trap mediation
        return SpawnResult()

    async def suspend(self, path: str) -> None:
        # TODO: suspend session to snapshot file
        pass

    async def resume(self, path: str) -> None:
        # TODO: resume session from snapshot file
        pass

    async def console(self) -> None:
        # TODO: open the PTY console master (break-glass surface)
        pass

    async def get_causal_delta_from_parent(self) -> CausalDelta:
        # TODO: fetch causal delta from parent session
        return CausalDelta()



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
    ) -> None:
        self._plugins: list[object] = []
        self._session: Session | None = None
        self._config: dict[str, Any] = {
            "vcpus": vcpus,
            "memory_mb": memory_mb,
            "kernel": kernel,
            "rootfs_erofs": rootfs_erofs,
            "tsc_locked": tsc_locked,
            "enable_watchdog": enable_watchdog,
            "driver": driver,
        }
        self._handlers: dict[str, list[Any]] = {}
        self._params_model: type | None = None

    def use(self, plugin: object) -> Self:
        self._plugins.append(plugin)
        return self

    def plugin(self, plugin: object) -> Self:
        self._plugins.append(plugin)
        return self

    def terminal(self, **kwargs: Any) -> Self:
        # TODO: configure the terminal plane
        self._config["terminal"] = kwargs
        return self

    def storage(self, **kwargs: Any) -> Self:
        # TODO: configure the storage plane
        self._config["storage"] = kwargs
        return self

    def wire(self, **kwargs: Any) -> Self:
        # TODO: configure the wire plane
        self._config["wire"] = kwargs
        return self

    def trap(
        self,
        pattern: str | TrapFilter,
        pre: Callable[..., Awaitable[Any]] | None = None,
        post: Callable[..., Awaitable[Any]] | None = None,
    ) -> Self:
        # TODO: register pre/post trap handlers into the pipeline
        self._handlers.setdefault("trap", []).append((pattern, pre, post))
        return self

    def register_custom_action(
        self,
        name: str,
        schema: dict[str, Any],
        handler: Callable[..., Awaitable[Any]],
    ) -> Self:
        # TODO: register a Patch(Custom{name, cbor}) action by name
        self._handlers.setdefault("custom_action", []).append((name, schema, handler))
        return self

    def register_vsock_service(
        self,
        kind: int,
        schema: dict[str, Any],
        handler: Callable[[bytes], Awaitable[bytes]],
    ) -> Self:
        # TODO: register a vsock service on plugin kind
        self._handlers.setdefault("vsock_service", []).append((kind, schema, handler))
        return self

    def _lifecycle_decorator(
        self, name: str
    ) -> Callable[[Callable[..., Awaitable[Any]]], Callable[..., Awaitable[Any]]]:
        def decorator(
            fn: Callable[..., Awaitable[Any]],
        ) -> Callable[..., Awaitable[Any]]:
            # TODO: register the lifecycle handler
            self._handlers.setdefault(name, []).append(fn)
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

    async def boot(self) -> Session:
        # TODO: Validate Pydantic dependencies and boot via PyO3
        return Session()

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
) -> KelirBuilder:
    return KelirBuilder(
        vcpus=vcpus,
        memory_mb=memory_mb,
        kernel=kernel,
        rootfs_erofs=rootfs_erofs,
        tsc_locked=tsc_locked,
        enable_watchdog=enable_watchdog,
        driver=driver,
    )


__all__ = [
    "Approval",
    "CausalDelta",
    "Deadline",
    "ExecutionReceipt",
    "FilterAction",
    "HandlerContext",
    "KelirBuilder",
    "PostExecAction",
    "ReceiptExtension",
    "ReconcileStats",
    "FinalReceipt",
    "Session",
    "SnapshotDescriptor",
    "SpawnResult",
    "TrapEvent",
    "TrapFilter",
    "Verdict",
    "VmmDriver",
    "builder",
    "kelir",
]
