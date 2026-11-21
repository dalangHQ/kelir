# see: cans/03-sdk-architecture.md#Python-SDK
# see: cans/02-plugin-model.md#Foreign-plugin-proxy
from __future__ import annotations

import importlib
import re
from dataclasses import dataclass, field
from datetime import timedelta
from typing import TYPE_CHECKING, Any, Callable, Generic, TypeVar

from kelir.sdk_errors import EConfig as EConfig
from kelir.generated.attest_telemetry import (
    ExecutionReceipt,
    SnapshotDescriptor,
)
# see: cans/03-sdk-architecture.md#Python-SDK
# see: cans/03-sdk-architecture.md#Python-SDK
KELIR_ADDON_ABI_VERSION = 1
NATIVE_BRIDGE_MODULES: tuple[str, ...] = ("kelir._kelir", "_kelir")

from enum import Enum

from kelir.plugin_fusion_flow import TrapFilterSnapshot


# see: cans/02-plugin-model.md#Unified-verdict
class Errno(str, Enum):
    EPERM = "EPERM"
    EACCES = "EACCES"
    ENOENT = "ENOENT"
    EINVAL = "EINVAL"
    ENOSPC = "ENOSPC"
    EROFS = "EROFS"
    ECONNREFUSED = "ECONNREFUSED"
    ENXIO = "ENXIO"
    EWOULDBLOCK = "EWOULDBLOCK"
    EFAULT = "EFAULT"
    EBUSY = "EBUSY"
    EXDEV = "EXDEV"
    EPIPE = "EPIPE"
    ENODATA = "ENODATA"
    EPROTO = "EPROTO"
    EPOLICY = "EPOLICY"
    ETOOLQUOTA = "ETOOLQUOTA"
    EWXVIOLATION = "EWXVIOLATION"
    EAFNOSUPPORT = "EAFNOSUPPORT"
    E_CONFIG = "EConfig"
    E_SECURITY_VIOLATION = "ESecurityViolation"
    E_UNKNOWN_KIND = "EUnknownKind"
    E_CLOCK_REGRESS = "EClockRegress"
    PATH_TRAVERSAL = "PathTraversal"
    ETIMEDOUT = "ETIMEDOUT"


# see: cans/11-protocol.md#Wire-messages
class Approval:
    pass


# see: cans/11-protocol.md#Wire-messages
class Handshake:
    pass


from dataclasses import dataclass


# see: cans/11-protocol.md#Wire-messages
@dataclass
class Deadline:
    value: int = 0


# see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
class StepVerdict(str, Enum):
    COMMIT = "commit"
    ROLLBACK = "rollback"
    YIELD_TO_HOST = "yield-to-host"


from typing import TypeVar

CtxT = TypeVar("CtxT")
ParamsT = TypeVar("ParamsT")


# see: cans/08-lifecycle.md#Suspend-and-resume
@dataclass
class TrapEvent:
    binary_path: str = ""
    argv: list[str] = field(default_factory=list)
    pid: int = 0
    ppid: int = 0
    tree_depth: int = 0
    cwd: str = ""
    binary_blake3: str = ""


CtxT = TypeVar("CtxT")
ParamsT = TypeVar("ParamsT")
# see: cans/03-sdk-architecture.md#Distribution-and-error-invariants

# see: cans/03-sdk-architecture.md#Distribution-and-error-invariants
class NativeAddonMissingError(EConfig):
    pass

# see: cans/03-sdk-architecture.md#Distribution-and-error-invariants

# see: cans/03-sdk-architecture.md#Distribution-and-error-invariants
class NativeAddonAbiMismatchError(EConfig):
    def __init__(self, expected: int, found: int) -> None:
        super().__init__(
            f"kelir native PyO3 extension (_kelir) ABI version mismatch: expected {expected}, found {found}"
        )
        self.expected_abi_version = expected
        self.found_abi_version = found

# see: cans/03-sdk-architecture.md#Python-SDK

# see: cans/03-sdk-architecture.md#Python-SDK
def _verify_native_bridge_abi(bridge: Any) -> None:
    abi_version = getattr(bridge, "abi_version", None)
    found = abi_version() if callable(abi_version) else None
    if found != KELIR_ADDON_ABI_VERSION:
        raise NativeAddonAbiMismatchError(
            KELIR_ADDON_ABI_VERSION, found if isinstance(found, int) else -1
        )

# see: cans/03-sdk-architecture.md#Python-SDK

# see: cans/03-sdk-architecture.md#Python-SDK
def _load_native_bridge() -> Any:
    for module in NATIVE_BRIDGE_MODULES:
        try:
            bridge = importlib.import_module(module)
        except ImportError:
            continue
        _verify_native_bridge_abi(bridge)
        return bridge
    # TODO: [bindings_py]: ship the precompiled _kelir wheel next to the package.; slop: dist ships without native extension, boot fails closed with NativeAddonMissingError
    raise NativeAddonMissingError(
        "kelir native PyO3 extension (_kelir) is missing: install the precompiled wheel"
    )

# see: cans/03-sdk-architecture.md#Python-SDK

# see: cans/03-sdk-architecture.md#Python-SDK
def _require_native(native: Any, what: str) -> Any:
    if native is None:
        raise NativeAddonMissingError(
            f"kelir native PyO3 bridge (_kelir) is missing: {what}"
        )
    return native

# see: cans/09-snapshots.md#Archives-and-portability

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

# see: cans/03-sdk-architecture.md#Python-SDK
def _bridge_safe_config(kind: str, kwargs: dict[str, Any]) -> dict[str, Any]:
    # TODO: [bindings_py]: snoop callbacks cross once the FFI callback bridge lands.; slop: snoop dropped silently when bridge absent
    # TODO: [bindings_py]: token shields and prompts cross once the FFI callback bridge lands.; slop: token shield/prompt dropped silently when bridge absent
    known = {
        "terminal": {
            "cols", "rows", "wireframe_grid", "sanitizer",
            "stream_engine",
        },
        "storage": {
            "idmap", "shared_caches", "lock_broker", "encapsulation",
            "dax_window", "blocking_wait_timeout_ms",
        },
        "wire": {
            "sinkhole", "ssh_agent_bridge", "l4_proxies",
            "tap_filter", "userspace_netstack", "default_egress",
            "divert_ports", "dial_timeout_ms", "block_abstract_unix_sockets",
            "dns_to_ip_pinning",
        },
    }[kind]
    unknown = set(kwargs) - known
    if unknown:
        raise EConfig(
            f"unknown {kind} plane config keys: {', '.join(sorted(unknown))}"
        )
    bridged: dict[str, Any] = {}
    for key, value in kwargs.items():
        if key in ("snoops", "token_shield", "prompts"):
            continue
        if key == "sinkhole" and value is not None:
            bridged["sinkhole"] = {
                "rules": [
                    {
                        "host": rule.host,
                        "methods": rule.methods,
                        "paths": rule.paths,
                        "allow": rule.allow,
                    }
                    for rule in value.rules
                ],
            }
        elif key == "wireframe_grid" and value is not None:
            bridged["wireframe_grid"] = (
                value
                if isinstance(value, dict)
                else {
                    "track_screen_matrix": value.track_screen_matrix,
                    "export_on_trap": value.export_on_trap,
                }
            )
        elif key == "sanitizer" and value is not None:
            bridged["sanitizer"] = (
                value
                if isinstance(value, dict)
                else {
                    "drop_osc52": value.drop_osc52,
                    "strip_title": value.strip_title,
                }
            )
        elif key == "shared_caches" and value is not None:
            bridged["shared_caches"] = [
                entry
                if isinstance(entry, dict)
                else {
                    "host_path": entry.host_path,
                    "guest_mount": entry.guest_mount,
                    "read_only": entry.read_only,
                }
                for entry in value
            ]
        elif key == "l4_proxies" and value is not None:
            bridged["l4_proxies"] = [
                {
                    "listen_port": proxy.listen_port,
                    "destination": proxy.destination,
                    "forward_unix_socket": proxy.forward_unix_socket,
                }
                for proxy in value
            ]
        elif key == "divert_ports" and value is not None:
            bridged["divert_ports"] = [
                entry
                if isinstance(entry, dict)
                else {"listen_port": entry[0], "forward_unix_socket": entry[1]}
                for entry in value
            ]
        else:
            bridged[key] = value
    return bridged

# see: cans/11-protocol.md#Wire-messages
# see: cans/11-protocol.md#Wire-messages
_condition_predicates: dict[int, Callable[[object], bool]] = {}
_next_condition_handle = 1

# see: cans/02-plugin-model.md#Unified-verdict

# see: cans/02-plugin-model.md#Unified-verdict
class Condition:

    def __init__(
        self, poll_interval_ms: int, predicate: Callable[[object], bool]
    ) -> None:
        global _next_condition_handle
        self.poll_interval_ms = poll_interval_ms
        self.handle = _next_condition_handle
        _next_condition_handle += 1
        _condition_predicates[self.handle] = predicate

# see: cans/11-protocol.md#Wire-messages

# see: cans/11-protocol.md#Wire-messages
def _suspend_until(until: Any) -> SuspendUntil | None:
    if until is None:
        return None
    if isinstance(until, type):
        raise EConfig(
            "suspend until must be an instance, e.g. Approval(), "
            "not the Approval class"
        )
    if not isinstance(until, (Approval, Deadline, Handshake, Condition)):
        raise EConfig(
            "suspend until must be Approval(), Deadline(ms), Handshake, or "
            f"Condition(poll_interval_ms, predicate), got {type(until).__name__}"
        )
    return until

# see: cans/11-protocol.md#Wire-framing

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

@dataclass
class PatchPayload:
    kind: str = "fs-path-redirect"
    target_path: str = ""
    target_host: str = ""
    target_port: int = 0
    replacement_signal: int = 0
    guidance: str = ""

    @classmethod
    def fs_path_redirect(cls, *, target_path: str) -> PatchPayload:
        return cls(target_path=target_path)

    @classmethod
    def net_redirect(
        cls, *, target_host: str, target_port: int, guidance: str = ""
    ) -> PatchPayload:
        return cls(
            kind="net-redirect",
            target_host=target_host,
            target_port=target_port,
            guidance=guidance,
        )

    @classmethod
    def signal_mutation(
        cls, *, replacement_signal: int, guidance: str = ""
    ) -> PatchPayload:
        return cls(
            kind="signal-mutation",
            replacement_signal=replacement_signal,
            guidance=guidance,
        )

    def to_wire(self) -> dict[str, Any]:
        if self.kind == "net-redirect":
            return {
                "kind": self.kind,
                "target_host": self.target_host,
                "target_port": self.target_port,
                "guidance": self.guidance,
            }
        if self.kind == "signal-mutation":
            return {
                "kind": self.kind,
                "replacement_signal": self.replacement_signal,
                "guidance": self.guidance,
            }
        return {"kind": self.kind, "target_path": self.target_path}


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
    def forward(cls) -> Verdict:
        return cls("forward")

    @classmethod
    def drop(cls, errno: str = "", reason: str = "") -> Verdict:
        # see: cans/02-plugin-model.md#Unified-verdict
        if errno:
            if errno == "E_PATH_TRAVERSAL":
                raise ValueError("use Errno.PATH_TRAVERSAL, not E_PATH_TRAVERSAL")
            try:
                Errno(errno)
            except ValueError:
                raise ValueError(f"unknown errno: {errno}") from None
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
        payload: Any = None,
    ) -> Verdict:
        # TODO: [bindings_py]: serialize to FsInodeAttr / Custom payload; slop: patch attr stays untyped Any, Rust rejects FsInodeAttr shape
        if args and attr is None:
            attr = args[0]
        if payload is not None:
            attr = payload
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

# see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)

# see: cans/plugins/step-eval.md#Invariants-and-policy-(step-eval)
class PostExecAction:
    def __init__(self, kind: str) -> None:
        self.kind = kind

    @classmethod
    def proceed(cls) -> PostExecAction:
        return cls("proceed")

    @classmethod
    def rollback_and_halt(cls) -> PostExecAction:
        return cls("rollback_and_halt")

    def to_step_verdict(self) -> StepVerdict:
        if self.kind == "proceed":
            return StepVerdict.COMMIT
        return StepVerdict.ROLLBACK


class TrapFilter:

    def __init__(
        self,
        binary: str | None = None,
        args: tuple[str, ...] = (),
        pattern: str | None = None,
        cwd: str | None = None,
        env: tuple[tuple[str, str], ...] = (),
        uid: int | None = None,
        argv: str | None = None,
    ) -> None:
        self.binary_name = binary
        self.args = args
        self.glob_pattern = pattern
        self.cwd_pattern = cwd
        self.env_matchers = env
        self.uid_filter = uid
        self.argv_pattern = argv

    @classmethod
    def binary(cls, name: str) -> TrapFilter:
        return cls(binary=name)

    @classmethod
    def pattern(cls, glob: str) -> TrapFilter:
        return cls(pattern=glob)

    def with_arg(self, arg: str) -> TrapFilter:
        return TrapFilter(
            self.binary_name,
            (*self.args, arg),
            self.glob_pattern,
            cwd=self.cwd_pattern,
            env=self.env_matchers,
            uid=self.uid_filter,
            argv=self.argv_pattern,
        )

    def with_cwd(self, cwd: str) -> TrapFilter:
        # see: cans/02-plugin-model.md#Filter-before-FFI-gate
        return TrapFilter(
            self.binary_name,
            self.args,
            self.glob_pattern,
            cwd=cwd,
            env=self.env_matchers,
            uid=self.uid_filter,
            argv=self.argv_pattern,
        )

    def with_env(self, key: str, value: str) -> TrapFilter:
        # see: cans/02-plugin-model.md#Filter-before-FFI-gate
        return TrapFilter(
            self.binary_name,
            self.args,
            self.glob_pattern,
            cwd=self.cwd_pattern,
            env=(*self.env_matchers, (key, value)),
            uid=self.uid_filter,
            argv=self.argv_pattern,
        )

    def with_uid(self, uid: int) -> TrapFilter:
        # see: cans/02-plugin-model.md#Filter-before-FFI-gate
        return TrapFilter(
            self.binary_name,
            self.args,
            self.glob_pattern,
            cwd=self.cwd_pattern,
            env=self.env_matchers,
            uid=uid,
            argv=self.argv_pattern,
        )

    def argv_matches(self, pattern: str | re.Pattern[str]) -> TrapFilter:
        # see: cans/02-plugin-model.md#Filter-before-FFI-gate
        return TrapFilter(
            self.binary_name,
            self.args,
            self.glob_pattern,
            cwd=self.cwd_pattern,
            env=self.env_matchers,
            uid=self.uid_filter,
            argv=pattern.pattern if isinstance(pattern, re.Pattern) else pattern,
        )

    def snapshot(self) -> TrapFilterSnapshot:
        # see: cans/02-plugin-model.md#Filter-before-FFI-gate
        return TrapFilterSnapshot(
            binary=self.binary_name,
            args=self.args,
            pattern=self.glob_pattern,
            cwd=self.cwd_pattern,
            env=self.env_matchers,
            uid=self.uid_filter,
            argv=self.argv_pattern,
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
        # TODO: [vr_06_virtio_shm_doorbell_fence]: map a zero-copy host memory segment via the PyO3 _kelir bridge.; slop: segment mapped without SeqCst fences, guest reads stale bytes
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: vm map_shared_region")

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
        # TODO: [vr_12_transport_af_vsock_fallback_parity]: write frame to vsock channel via the PyO3 _kelir bridge.; slop: vsock frame bytes differ from SPSC framing
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: vsock write")

    async def read(self) -> bytes:
        # TODO: [vr_12_transport_af_vsock_fallback_parity]: read frame from vsock channel via the PyO3 _kelir bridge.; slop: vsock read returns SPSC-incompatible framing
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: vsock read")


class VsockHandle:
    async def channel(self, kind: int) -> VsockChannel:
        # TODO: [vr_12_transport_af_vsock_fallback_parity]: open duplex byte stream on vsock kind via the PyO3 _kelir bridge.; slop: duplex stream framing diverges from SPSC carrier
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: vsock channel")


class FsHandle:
    async def warm_cache(self, paths: list[str]) -> None:
        # TODO: [storage_plane]: warm shared caches into the guest via the PyO3 _kelir bridge.; slop: warm_cache no-ops, guest reads cold upper
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: fs warm_cache")


class TerminalHandle:
    def __init__(self, native: Any = None) -> None:
        self._native = native

    def notice(self, text: str) -> None:
        # TODO: [terminal_plane]: publish TerminalNotice to the event bus via the PyO3 _kelir bridge.; slop: TerminalNotice never published, terminal state diverges
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: terminal notice")

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

    async def snapshot(
        self, lineage: Any, descriptor: SnapshotDescriptor
    ) -> int:
        native = _require_native(self._native, "checkpoint snapshot")
        return int(native.snapshot(lineage, _snapshot_to_dict(descriptor)))

    async def fork(self, node_id: int, lineage: Any) -> None:
        native = _require_native(self._native, "checkpoint fork")
        native.fork(node_id, lineage)

    async def branch_from(self, node_id: int, lineage: Any) -> None:
        native = _require_native(self._native, "checkpoint branch_from")
        native.branch_from(node_id, lineage)

    async def rollback_node(
        self, node_id: int, guest_monotonic_ns: int = 0
    ) -> SnapshotDescriptor:
        native = _require_native(self._native, "checkpoint rollback_node")
        return _snapshot_from_dict(
            native.rollback_node(node_id, guest_monotonic_ns)
        )

    async def compact(self, lineage: Any) -> int:
        native = _require_native(self._native, "checkpoint compact")
        return int(native.compact(lineage))

    async def revert_step(self, slot: int) -> None:
        native = _require_native(self._native, "checkpoint revert_step")
        native.revert_step(slot)

    async def rollback(self, snapshot_id: str) -> None:
        native = _require_native(self._native, "checkpoint rollback")
        native.rollback(snapshot_id)


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
        # TODO: [lifecycle_checkpoint]: return the most recently marked checkpoint via the PyO3 _kelir bridge.; slop: checkpoint lookup returns stale or fabricated descriptor
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: step last")


class HandlerContext(Generic[CtxT, ParamsT]):
    def __init__(
        self,
        trap: TrapEvent | None = None,
        cap: Caps | None = None,
        ctx: CtxT | None = None,
        params: ParamsT | None = None,
        session: Session | None = None,
    ) -> None:
        self.trap: TrapEvent = trap if trap is not None else TrapEvent()
        self._session: Session | None = session
        if session is None:
            from kelir.lifecycle_session_flow import Session as _S
            session = _S()
        self.vm = VmHandle(session)
        self.checkpoint = CheckpointHandle()
        self.fs = FsHandle()
        self.terminal = TerminalHandle()
        self.ctx: CtxT | None = ctx
        self.step = StepHandle()
        if cap is None:
            from kelir.service_caps_flow import Caps as _C
            cap = _C()
        self.cap = cap
        self.params: ParamsT | None = params

    def has_valid_operator_lease(self) -> bool:
        # see: cans/03-sdk-architecture.md#Python-SDK
        session = self._session
        return (
            session is not None
            and session._operator_lease_token is not None
        )

    async def receipt(self) -> ExecutionReceipt:
        # TODO: [ar_01_execution_receipt_blake3_binding]: fetch the trap's execution receipt via the PyO3 _kelir bridge.; slop: receipt fabricated without binary BLAKE3 binding
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: trap receipt")

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

    # see: cans/03-sdk-architecture.md#Service-lookup-across-languages
    def cap_for(self, name: str) -> Any | None:
        return getattr(self.cap, name, None)

# see: cans/08-lifecycle.md#Detached-lifecycle

# see: cans/08-lifecycle.md#Detached-lifecycle
def relay_socket_path(session_id: str) -> str:
    bridge = _load_native_bridge()
    try:
        return str(bridge.relay_socket_path(session_id))
    except Exception as exc:
        raise EConfig(str(exc)) from exc

