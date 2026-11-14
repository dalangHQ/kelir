# see: cans/03-sdk-architecture.md#Python-SDK
# see: cans/02-plugin-model.md#Foreign-plugin-proxy
from __future__ import annotations

import importlib
from dataclasses import dataclass, field
from datetime import timedelta
from enum import Enum
from typing import TYPE_CHECKING, Any, Generic, TypeVar

CtxT = TypeVar("CtxT")
ParamsT = TypeVar("ParamsT")

if TYPE_CHECKING:
    from kelir.model_plugin import BasePlugin

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
    LifecycleTag,
    PluginRegistration,
    RingEvent,
    TrapFilterSnapshot,
)


# see: cans/03-sdk-architecture.md#Python-SDK
KELIR_ADDON_ABI_VERSION = 1

# see: cans/03-sdk-architecture.md#Python-SDK
NATIVE_BRIDGE_MODULES: tuple[str, ...] = ("kelir._kelir", "_kelir")


# see: cans/03-sdk-architecture.md#Distribution-and-error-invariants
class NativeAddonMissingError(EConfig):
    pass


# see: cans/03-sdk-architecture.md#Distribution-and-error-invariants
class NativeAddonAbiMismatchError(EConfig):
    def __init__(self, expected: int, found: int) -> None:
        super().__init__(
            f"kelir native PyO3 extension (_kelir) ABI version mismatch: expected {expected}, found {found}"
        )
        self.expected_abi_version = expected
        self.found_abi_version = found


# see: cans/03-sdk-architecture.md#Python-SDK
def _verify_native_bridge_abi(bridge: Any) -> None:
    abi_version = getattr(bridge, "abi_version", None)
    found = abi_version() if callable(abi_version) else None
    if found != KELIR_ADDON_ABI_VERSION:
        raise NativeAddonAbiMismatchError(
            KELIR_ADDON_ABI_VERSION, found if isinstance(found, int) else -1
        )


# see: cans/03-sdk-architecture.md#Python-SDK
def _load_native_bridge() -> Any:
    for module in NATIVE_BRIDGE_MODULES:
        try:
            bridge = importlib.import_module(module)
        except ImportError:
            continue
        _verify_native_bridge_abi(bridge)
        return bridge
    # TODO (L3) [bindings_py]: ship the precompiled _kelir wheel next to the package.; slop: dist ships without native extension, boot fails closed with NativeAddonMissingError
    raise NativeAddonMissingError(
        "kelir native PyO3 extension (_kelir) is missing: install the precompiled wheel (L3)"
    )


# see: cans/03-sdk-architecture.md#Python-SDK
def _require_native(native: Any, what: str) -> Any:
    if native is None:
        raise NativeAddonMissingError(
            f"kelir native PyO3 bridge (_kelir) is missing: {what} (L3)"
        )
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
    # TODO (L3) [bindings_py]: token shields, prompts, and snoop callbacks cross once the FFI callback bridge lands.; slop: token shield/prompt/snoop dropped silently when bridge absent
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


# see: cans/11-protocol.md#Wire-messages
SuspendUntil = Approval | Deadline | Handshake


# see: cans/11-protocol.md#Wire-messages
def _suspend_until(until: Any) -> SuspendUntil | None:
    if until is None:
        return None
    if isinstance(until, type):
        raise EConfig(
            "suspend until must be an instance, e.g. Approval(), "
            "not the Approval class"
        )
    if not isinstance(until, (Approval, Deadline, Handshake)):
        raise EConfig(
            "suspend until must be Approval(), Deadline(ms), or Handshake, "
            f"got {type(until).__name__}"
        )
    return until


# see: cans/artifacts/code/vmm-driver-trait.rs#SuspendReason
class ExternalSuspendKind(str, Enum):
    HUMAN_AUTHORIZATION_HOLD = "human-authorization-hold"
    WIRE_CONNECTION_PARKING = "wire-connection-parking"
    STEP_EVALUATION_YIELD = "step-evaluation-yield"
    HOST_POWER_EVENT_SNAPSHOT = "host-power-event-snapshot"


# see: cans/artifacts/code/vmm-driver-trait.rs#SuspendReason
@dataclass
class External:
    kind: ExternalSuspendKind


# see: cans/artifacts/code/vmm-driver-trait.rs#SuspendReason
class ResourcePressure:
    pass


# see: cans/artifacts/code/vmm-driver-trait.rs#SuspendReason
SuspendReason = External | ResourcePressure


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
        # TODO (L3) [bindings_py]: serialize to FsInodeAttr / Custom payload; slop: patch attr stays untyped Any, Rust rejects FsInodeAttr shape
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
    def suspend(cls, until: SuspendUntil | None = None) -> Verdict:
        return cls("suspend", until=_suspend_until(until))

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
    def suspend(cls, until: SuspendUntil | None = None) -> FilterAction:
        return cls("suspend", until=_suspend_until(until))

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

    def __init__(
        self,
        binary: str | None = None,
        args: tuple[str, ...] = (),
        pattern: str | None = None,
    ) -> None:
        self.binary_name = binary
        self.args = args
        self.glob_pattern = pattern

    @classmethod
    def binary(cls, name: str) -> TrapFilter:
        return cls(binary=name)

    @classmethod
    def pattern(cls, glob: str) -> TrapFilter:
        return cls(pattern=glob)

    def with_arg(self, arg: str) -> TrapFilter:
        return TrapFilter(self.binary_name, (*self.args, arg), self.glob_pattern)

    def snapshot(self) -> TrapFilterSnapshot:
        # see: cans/02-plugin-model.md#Filter-before-FFI-gate
        return TrapFilterSnapshot(
            binary=self.binary_name, args=self.args, pattern=self.glob_pattern
        )


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
from kelir.service_caps import (
    Caps as Caps,
    ContainerCap as ContainerCap,
    DbCap as DbCap,
    LedgerCap as LedgerCap,
    LedgerFinalizeResult as LedgerFinalizeResult,
    RepoPolicyCap as RepoPolicyCap,
    Savepoint as Savepoint,
    TransactionCap as TransactionCap,
    TransactionScope as TransactionScope,
)


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
        # TODO (L3) [vr_06_virtio_shm_doorbell_fence]: map a zero-copy host memory segment via the PyO3 _kelir bridge.; slop: segment mapped without SeqCst fences, guest reads stale bytes
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
        # TODO (L3) [vr_12_transport_af_vsock_fallback_parity]: write frame to vsock channel via the PyO3 _kelir bridge.; slop: vsock frame bytes differ from SPSC framing
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: vsock write (L3)")

    async def read(self) -> bytes:
        # TODO (L3) [vr_12_transport_af_vsock_fallback_parity]: read frame from vsock channel via the PyO3 _kelir bridge.; slop: vsock read returns SPSC-incompatible framing
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: vsock read (L3)")


class VsockHandle:
    async def channel(self, kind: int) -> VsockChannel:
        # TODO (L3) [vr_12_transport_af_vsock_fallback_parity]: open duplex byte stream on vsock kind via the PyO3 _kelir bridge.; slop: duplex stream framing diverges from SPSC carrier
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: vsock channel (L3)")


class FsHandle:
    async def warm_cache(self, paths: list[str]) -> None:
        # TODO (L3) [storage_plane]: warm shared caches into the guest via the PyO3 _kelir bridge.; slop: warm_cache no-ops, guest reads cold upper
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: fs warm_cache (L3)")


class TerminalHandle:
    def __init__(self, native: Any = None) -> None:
        self._native = native

    def notice(self, text: str) -> None:
        # TODO (L3) [terminal_plane]: publish TerminalNotice to the event bus via the PyO3 _kelir bridge.; slop: TerminalNotice never published, terminal state diverges
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
        # TODO (L3) [lifecycle_checkpoint]: return the most recently marked checkpoint via the PyO3 _kelir bridge.; slop: checkpoint lookup returns stale or fabricated descriptor
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: step last (L3)")



class HandlerContext(Generic[CtxT, ParamsT]):
    def __init__(
        self,
        trap: TrapEvent | None = None,
        cap: Caps | None = None,
        ctx: CtxT | None = None,
        params: ParamsT | None = None,
    ) -> None:
        self.trap: TrapEvent = trap if trap is not None else TrapEvent()
        self.vm = VmHandle(Session())
        self.checkpoint = CheckpointHandle()
        self.fs = FsHandle()
        self.terminal = TerminalHandle()
        self.ctx: CtxT | None = ctx
        self.step = StepHandle()
        self.cap = cap if cap is not None else Caps()
        self.params: ParamsT | None = params

    async def receipt(self) -> ExecutionReceipt:
        # TODO (L3) [ar_01_execution_receipt_blake3_binding]: fetch the trap's execution receipt via the PyO3 _kelir bridge.; slop: receipt fabricated without binary BLAKE3 binding
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: trap receipt (L3)")

    # see: cans/02-plugin-model.md#Unified-verdict
    def pass_(self) -> Verdict:
        return Verdict.pass_()

    # see: cans/02-plugin-model.md#Unified-verdict
    def drop(self, errno: str = "", reason: str = "") -> Verdict:
        return Verdict.drop(errno, reason)

    # see: cans/02-plugin-model.md#Unified-verdict
    def patch(
        self,
        *args: Any,
        binary: str | None = None,
        argv: list[str] | None = None,
        env: dict[str, str] | None = None,
        unset_env: list[str] | None = None,
        name: str | None = None,
        cbor: dict[str, Any] | None = None,
        attr: Any = None,
    ) -> Verdict:
        return Verdict.patch(
            *args,
            binary=binary,
            argv=argv,
            env=env,
            unset_env=unset_env,
            name=name,
            cbor=cbor,
            attr=attr,
        )

    # see: cans/02-plugin-model.md#Unified-verdict
    def suspend(self, until: SuspendUntil | None = None) -> Verdict:
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


from kelir.lifecycle_session import Session as Session
from kelir.kelir_builder import (
    KelirBuilder as KelirBuilder,
    VsockServiceEntry as VsockServiceEntry,
    builder as builder,
    kelir as kelir,
)


__all__ = [
    "Approval",
    "Caps",
    "CausalDelta",
    "Channel",
    "CheckpointHandle",
    "ContainerCap",
    "CtxT",
    "DbCap",
    "Deadline",
    "ExecStream",
    "ExecveMutation",
    "ExecutionReceipt",
    "ExitKind",
    "External",
    "ExternalSuspendKind",
    "FilterAction",
    "GuestClock",
    "Handshake",
    "HandlerContext",
    "HostHandle",
    "KelirBuilder",
    "LedgerCap",
    "LedgerFinalizeResult",
    "MsgType",
    "ParamsT",
    "PhaseTimings",
    "PostExecAction",
    "ReceiptExtension",
    "RepoPolicyCap",
    "ResourcePressure",
    "CommitStats",
    "Sandbox",
    "Savepoint",
    "Session",
    "SnapshotDescriptor",
    "SpawnResult",
    "StreamKind",
    "SuspendReason",
    "SuspendUntil",
    "TransactionCap",
    "TransactionScope",
    "TrapEvent",
    "TrapFilter",
    "Verdict",
    "builder",
    "kelir",
    "relay_socket_path",
]
