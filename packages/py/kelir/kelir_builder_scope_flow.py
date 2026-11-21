# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
from __future__ import annotations

import inspect
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, Awaitable, Callable, Protocol, Self

from kelir.sdk_errors import EConfig

if TYPE_CHECKING:
    from kelir.kelir_builder_flow import KelirBuilder


Handler = Callable[..., Any]

_FS_CONFIG_KEYS: frozenset[str] = frozenset(
    {"lock_broker", "carryover_bytes", "scan_engine", "snoops"}
)

@dataclass
class VsockServiceEntry:
    port: int
    service: str
    schema: dict[str, Any]
    handler: Callable[[bytes], Awaitable[bytes]] | None = None


# see: cans/02-plugin-model.md#Nested-scopes
_SCOPE_VERBS: dict[str, tuple[str, ...]] = {
    "trap": ("on_trap", "on_process_exit", "on_trap_deny", "on_signal", "on_fork"),
    "fs": (
        "on_fs_lookup",
        "on_fs_read",
        "on_fs_write",
        "on_fs_lock",
        "on_fs_unlink",
        "on_fs_rename",
        "on_fs_setattr",
        "on_fs_symlink",
        "on_fs_mkdir",
        "on_fs_readdir",
        "on_fs_readlink",
    ),
    "net": ("on_net_outbound", "on_net_inbound", "on_socket_connect", "on_dns_query", "on_socket_bind"),
    "console": ("on_console_input", "on_console_output"),
    "exec": ("on_exec_stdout", "on_exec_stderr", "on_exec_stdin"),
    "host": (
        "on_host_lsm_exec",
        "on_host_pressure",
        "on_host_keyring_access",
        "on_host_preempt",
        "on_host_containment_breach",
    ),
    "lifecycle": (
        "on_vm_init",
        "on_vm_ready",
        "on_vm_shutdown",
        "on_pre_commit",
        "on_post_commit",
    ),
    "telemetry": ("on_memory_pressure", "on_heartbeat", "on_watchdog"),
    "actions": ("register_custom_action",),
    "streams": ("register_stream_kind",),
    "vsock": ("vsock_service",),
    "params": ("params",),
    "deny": ("on_trap_deny",),
}


# see: cans/02-plugin-model.md#Nested-scopes
class TrapScopeProtocol(Protocol):
    def on_trap(self, handler: Handler) -> TrapScopeProtocol: ...
    def on_process_exit(self, handler: Handler) -> TrapScopeProtocol: ...
    def on_trap_deny(self, handler: Handler) -> TrapScopeProtocol: ...
    def on_signal(self, handler: Handler) -> TrapScopeProtocol: ...
    def on_fork(self, handler: Handler) -> TrapScopeProtocol: ...
    def require(self, plugin: type | str) -> TrapScopeProtocol: ...
    def timeout(self, seconds: float) -> TrapScopeProtocol: ...
    def priority(self, priority: int) -> TrapScopeProtocol: ...
    def end(self) -> KelirBuilder: ...


# see: cans/02-plugin-model.md#Nested-scopes
class FsScopeProtocol(Protocol):
    def on_fs_lookup(self, handler: Handler) -> FsScopeProtocol: ...
    def on_fs_read(self, handler: Handler) -> FsScopeProtocol: ...
    def on_fs_write(self, handler: Handler) -> FsScopeProtocol: ...
    def on_fs_lock(self, handler: Handler) -> FsScopeProtocol: ...
    def on_fs_unlink(self, handler: Handler) -> FsScopeProtocol: ...
    def on_fs_rename(self, handler: Handler) -> FsScopeProtocol: ...
    def on_fs_setattr(self, handler: Handler) -> FsScopeProtocol: ...
    def on_fs_symlink(self, handler: Handler) -> FsScopeProtocol: ...
    def on_fs_mkdir(self, handler: Handler) -> FsScopeProtocol: ...
    def on_fs_readdir(self, handler: Handler) -> FsScopeProtocol: ...
    def on_fs_readlink(self, handler: Handler) -> FsScopeProtocol: ...
    def config(self, **kwargs: Any) -> FsScopeProtocol: ...
    def require(self, plugin: type | str) -> FsScopeProtocol: ...
    def fork_with(self, fn: Callable[[KelirBuilder], KelirBuilder]) -> FsScopeProtocol: ...
    def timeout(self, seconds: float) -> FsScopeProtocol: ...
    def priority(self, priority: int) -> FsScopeProtocol: ...
    def rules(self, rules: dict[str, Any]) -> FsScopeProtocol: ...
    def end(self) -> KelirBuilder: ...


# see: cans/02-plugin-model.md#Nested-scopes
class NetScopeProtocol(Protocol):
    def on_net_outbound(self, handler: Handler) -> NetScopeProtocol: ...
    def on_net_inbound(self, handler: Handler) -> NetScopeProtocol: ...
    def on_socket_connect(self, handler: Handler) -> NetScopeProtocol: ...
    def on_dns_query(self, handler: Handler) -> NetScopeProtocol: ...
    def on_socket_bind(self, handler: Handler) -> NetScopeProtocol: ...
    def require(self, plugin: type | str) -> NetScopeProtocol: ...
    def timeout(self, seconds: float) -> NetScopeProtocol: ...
    def priority(self, priority: int) -> NetScopeProtocol: ...
    def end(self) -> KelirBuilder: ...


# see: cans/02-plugin-model.md#Nested-scopes
class ConsoleScopeProtocol(Protocol):
    def on_console_input(self, handler: Handler) -> ConsoleScopeProtocol: ...
    def on_console_output(self, handler: Handler) -> ConsoleScopeProtocol: ...
    def require(self, plugin: type | str) -> ConsoleScopeProtocol: ...
    def timeout(self, seconds: float) -> ConsoleScopeProtocol: ...
    def priority(self, priority: int) -> ConsoleScopeProtocol: ...
    def end(self) -> KelirBuilder: ...


# see: cans/02-plugin-model.md#Nested-scopes
class ExecScopeProtocol(Protocol):
    def on_exec_stdout(self, handler: Handler) -> ExecScopeProtocol: ...
    def on_exec_stderr(self, handler: Handler) -> ExecScopeProtocol: ...
    def on_exec_stdin(self, handler: Handler) -> ExecScopeProtocol: ...
    def require(self, plugin: type | str) -> ExecScopeProtocol: ...
    def timeout(self, seconds: float) -> ExecScopeProtocol: ...
    def priority(self, priority: int) -> ExecScopeProtocol: ...
    def end(self) -> KelirBuilder: ...


# see: cans/02-plugin-model.md#Nested-scopes
class HostScopeProtocol(Protocol):
    def on_host_lsm_exec(self, handler: Handler) -> HostScopeProtocol: ...
    def on_host_pressure(self, handler: Handler) -> HostScopeProtocol: ...
    def on_host_keyring_access(self, handler: Handler) -> HostScopeProtocol: ...
    def on_host_preempt(self, handler: Handler) -> HostScopeProtocol: ...
    def on_host_containment_breach(self, handler: Handler) -> HostScopeProtocol: ...
    def require(self, plugin: type | str) -> HostScopeProtocol: ...
    def timeout(self, seconds: float) -> HostScopeProtocol: ...
    def priority(self, priority: int) -> HostScopeProtocol: ...
    def end(self) -> KelirBuilder: ...


# see: cans/02-plugin-model.md#Nested-scopes
class LifecycleScopeProtocol(Protocol):
    def on_vm_init(self, handler: Handler) -> LifecycleScopeProtocol: ...
    def on_vm_ready(self, handler: Handler) -> LifecycleScopeProtocol: ...
    def on_vm_shutdown(self, handler: Handler) -> LifecycleScopeProtocol: ...
    def on_pre_commit(self, handler: Handler) -> LifecycleScopeProtocol: ...
    def on_post_commit(self, handler: Handler) -> LifecycleScopeProtocol: ...
    def require(self, plugin: type | str) -> LifecycleScopeProtocol: ...
    def timeout(self, seconds: float) -> LifecycleScopeProtocol: ...
    def priority(self, priority: int) -> LifecycleScopeProtocol: ...
    def end(self) -> KelirBuilder: ...


# see: cans/02-plugin-model.md#Nested-scopes
class TelemetryScopeProtocol(Protocol):
    def on_memory_pressure(self, handler: Handler) -> TelemetryScopeProtocol: ...
    def on_heartbeat(self, handler: Handler) -> TelemetryScopeProtocol: ...
    def on_watchdog(self, handler: Handler) -> TelemetryScopeProtocol: ...
    def require(self, plugin: type | str) -> TelemetryScopeProtocol: ...
    def timeout(self, seconds: float) -> TelemetryScopeProtocol: ...
    def priority(self, priority: int) -> TelemetryScopeProtocol: ...
    def end(self) -> KelirBuilder: ...


# see: cans/02-plugin-model.md#Nested-scopes
class ActionsScopeProtocol(Protocol):
    def register(self, name_or_kind: str | int, schema: dict[str, Any], handler: Handler) -> ActionsScopeProtocol: ...
    def require(self, plugin: type | str) -> ActionsScopeProtocol: ...
    def timeout(self, seconds: float) -> ActionsScopeProtocol: ...
    def priority(self, priority: int) -> ActionsScopeProtocol: ...
    def end(self) -> KelirBuilder: ...


# see: cans/02-plugin-model.md#Nested-scopes
class StreamsScopeProtocol(Protocol):
    def register(self, name_or_kind: str | int, schema: dict[str, Any], handler: Handler) -> StreamsScopeProtocol: ...
    def require(self, plugin: type | str) -> StreamsScopeProtocol: ...
    def timeout(self, seconds: float) -> StreamsScopeProtocol: ...
    def priority(self, priority: int) -> StreamsScopeProtocol: ...
    def end(self) -> KelirBuilder: ...


# see: cans/02-plugin-model.md#Nested-scopes
class VsockScopeProtocol(Protocol):
    def service(self, port: int, service: str, handler: Handler) -> VsockScopeProtocol: ...
    def require(self, plugin: type | str) -> VsockScopeProtocol: ...
    def timeout(self, seconds: float) -> VsockScopeProtocol: ...
    def priority(self, priority: int) -> VsockScopeProtocol: ...
    def end(self) -> KelirBuilder: ...


# see: cans/02-plugin-model.md#Nested-scopes
class ParamsScopeProtocol(Protocol):
    def model(self, model: type) -> ParamsScopeProtocol: ...
    def require(self, plugin: type | str) -> ParamsScopeProtocol: ...
    def timeout(self, seconds: float) -> ParamsScopeProtocol: ...
    def priority(self, priority: int) -> ParamsScopeProtocol: ...
    def end(self) -> KelirBuilder: ...


# see: cans/02-plugin-model.md#Nested-scopes
class DenyScopeProtocol(Protocol):
    def on_trap_deny(self, handler: Handler) -> DenyScopeProtocol: ...
    def require(self, plugin: type | str) -> DenyScopeProtocol: ...
    def timeout(self, seconds: float) -> DenyScopeProtocol: ...
    def priority(self, priority: int) -> DenyScopeProtocol: ...
    def end(self) -> KelirBuilder: ...


# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
class _Scope:
    def __init__(self, builder: KelirBuilder, name: str, verbs: tuple[str, ...]) -> None:
        self._builder = builder
        self._name = name
        self._verbs = frozenset(verbs)

    def _scoped_on(self, verb: str, handler: Handler) -> Self:
        if verb not in self._verbs:
            raise EConfig(f".{verb} is not in this scope")
        self._builder._on(verb, handler)
        return self

    def on_trap_deny(
        self,
        handler: Handler,
    ) -> Self:
        if "on_trap_deny" not in self._verbs:
            raise EConfig(".on_trap_deny is not in this scope")
        # see: cans/02-plugin-model.md#Unified-verdict
        self._builder._on_trap_deny(handler)
        return self

    def register(
        self, name_or_kind: str | int, schema: dict[str, Any], handler: Handler
    ) -> Self:
        if "register_custom_action" in self._verbs and isinstance(name_or_kind, str):
            self._builder._register_custom_action(name_or_kind, schema, handler)
        elif "register_stream_kind" in self._verbs and isinstance(name_or_kind, int):
            self._builder._register_stream_kind(name_or_kind, schema, handler)
        else:
            raise EConfig(".register is not in this scope")
        return self

    def service(self, port: int, service: str, handler: Handler) -> Self:
        if "vsock_service" not in self._verbs:
            raise EConfig(".service is not in this scope")
        self._builder._vsock_service(
            VsockServiceEntry(port=port, service=service, schema={}, handler=handler)
        )
        return self

    def model(self, model: type) -> Self:
        if "params" not in self._verbs:
            raise EConfig(".model is not in this scope")
        self._builder._set_params_model(model)
        return self

    # see: cans/02-plugin-model.md#Nested-scopes
    def config(self, **kwargs: Any) -> Self:
        if "on_fs_lookup" not in self._verbs:
            raise EConfig(".config is not in this scope")
        if self._builder._mode == "plugin":
            self._builder._raise_host_only("fs.config")
        unknown = set(kwargs) - _FS_CONFIG_KEYS
        if unknown:
            raise EConfig(f"unknown fs config keys: {', '.join(sorted(unknown))}")
        validated: dict[str, Any] = {}
        if "lock_broker" in kwargs:
            broker = kwargs["lock_broker"]
            if not isinstance(broker, dict):
                raise EConfig(".config(lock_broker=...) expects a dict")
            broker_unknown = set(broker) - {"globs", "non_blocking"}
            if broker_unknown:
                raise EConfig(
                    f"unknown lock_broker keys: {', '.join(sorted(broker_unknown))}"
                )
            globs = broker.get("globs", [])
            if not isinstance(globs, list) or not all(
                isinstance(item, str) for item in globs
            ):
                raise EConfig(".config(lock_broker.globs=...) expects a list of str")
            non_blocking = broker.get("non_blocking", False)
            if not isinstance(non_blocking, bool):
                raise EConfig(".config(lock_broker.non_blocking=...) expects a bool")
            validated["lock_broker"] = {
                "globs": list(globs),
                "non_blocking": non_blocking,
            }
        if "carryover_bytes" in kwargs:
            carryover = kwargs["carryover_bytes"]
            if isinstance(carryover, bool) or not isinstance(carryover, int):
                raise EConfig(".config(carryover_bytes=...) expects an int")
            if carryover < 0:
                raise EConfig(".config(carryover_bytes=...) expects a non-negative int")
            validated["carryover_bytes"] = carryover
        if "scan_engine" in kwargs:
            if not isinstance(kwargs["scan_engine"], str):
                raise EConfig(".config(scan_engine=...) expects a str")
            validated["scan_engine"] = kwargs["scan_engine"]
        if "snoops" in kwargs:
            if not isinstance(kwargs["snoops"], list):
                raise EConfig(".config(snoops=...) expects a list")
            validated["snoops"] = list(kwargs["snoops"])
        # see: cans/03-sdk-architecture.md#Python-SDK (apply: the stored fs config in the live lock broker, scan engine, and carryover path. deferred)
        self._builder._fs_scope["config"] = validated
        return self

    def require(self, plugin: type | str) -> Self:
        self._builder.require(plugin)
        return self

    def fork_with(self, fn: Callable[[KelirBuilder], KelirBuilder]) -> Self:
        if "on_fs_lookup" not in self._verbs:
            raise EConfig(".fork_with is not in this scope")
        self._builder._fs_scope["fork_with"] = fn
        return self

    def timeout(self, seconds: float) -> Self:
        if isinstance(seconds, bool) or not isinstance(seconds, (int, float)):
            raise EConfig(".timeout(seconds) expects a number")
        if seconds < 0:
            raise EConfig(".timeout(seconds) expects a non-negative number")
        self._builder._scope_timeouts[self._name] = seconds
        return self

    def priority(self, priority: int) -> Self:
        if isinstance(priority, bool) or not isinstance(priority, int):
            raise EConfig(".priority(priority) expects an int")
        self._builder._scope_priorities[self._name] = priority
        return self

    def rules(self, rules: dict[str, Any]) -> Self:
        if "on_fs_lookup" not in self._verbs:
            raise EConfig(".rules is not in this scope")
        if not isinstance(rules, dict):
            raise EConfig(".rules(rules) expects a dict")
        unknown = set(rules) - {"read", "write"}
        if unknown:
            raise EConfig(f"unknown fs rules keys: {', '.join(sorted(unknown))}")
        validated: dict[str, Any] = {}
        if "read" in rules:
            read = rules["read"]
            if not isinstance(read, dict):
                raise EConfig(".rules(read=...) expects a dict")
            read_unknown = set(read) - {"allow", "deny"}
            if read_unknown:
                raise EConfig(
                    f"unknown fs read rules keys: {', '.join(sorted(read_unknown))}"
                )
            for key in ("allow", "deny"):
                if key in read and (
                    not isinstance(read[key], list)
                    or not all(isinstance(item, str) for item in read[key])
                ):
                    raise EConfig(f".rules(read.{key}=...) expects a list of str")
            validated["read"] = {key: list(read[key]) for key in read}
        if "write" in rules:
            write = rules["write"]
            if not isinstance(write, dict):
                raise EConfig(".rules(write=...) expects a dict")
            write_unknown = set(write) - {"mask"}
            if write_unknown:
                raise EConfig(
                    f"unknown fs write rules keys: {', '.join(sorted(write_unknown))}"
                )
            if "mask" in write and (
                not isinstance(write["mask"], list)
                or not all(isinstance(item, str) for item in write["mask"])
            ):
                raise EConfig(".rules(write.mask=...) expects a list of str")
            validated["write"] = {key: list(write[key]) for key in write}
        # see: cans/03-sdk-architecture.md#Python-SDK (apply: the stored fs rules in the live read/write enforcement path. deferred)
        self._builder._fs_scope["rules"] = validated
        return self

    def end(self) -> KelirBuilder:
        return self._builder

    def on_trap(self, handler: Handler) -> Self:
        return self._scoped_on("on_trap", handler)

    def on_process_exit(self, handler: Handler) -> Self:
        return self._scoped_on("on_process_exit", handler)

    def on_signal(self, handler: Handler) -> Self:
        return self._scoped_on("on_signal", handler)

    def on_fork(self, handler: Handler) -> Self:
        return self._scoped_on("on_fork", handler)

    def on_fs_lookup(self, handler: Handler) -> Self:
        return self._scoped_on("on_fs_lookup", handler)

    def on_fs_read(self, handler: Handler) -> Self:
        return self._scoped_on("on_fs_read", handler)

    def on_fs_write(self, handler: Handler) -> Self:
        return self._scoped_on("on_fs_write", handler)

    def on_fs_lock(self, handler: Handler) -> Self:
        return self._scoped_on("on_fs_lock", handler)

    def on_fs_unlink(self, handler: Handler) -> Self:
        return self._scoped_on("on_fs_unlink", handler)

    def on_fs_rename(self, handler: Handler) -> Self:
        return self._scoped_on("on_fs_rename", handler)

    def on_fs_setattr(self, handler: Handler) -> Self:
        return self._scoped_on("on_fs_setattr", handler)

    def on_fs_symlink(self, handler: Handler) -> Self:
        return self._scoped_on("on_fs_symlink", handler)

    def on_fs_mkdir(self, handler: Handler) -> Self:
        return self._scoped_on("on_fs_mkdir", handler)

    def on_fs_readdir(self, handler: Handler) -> Self:
        return self._scoped_on("on_fs_readdir", handler)

    def on_fs_readlink(self, handler: Handler) -> Self:
        return self._scoped_on("on_fs_readlink", handler)

    def on_net_outbound(self, handler: Handler) -> Self:
        return self._scoped_on("on_net_outbound", handler)

    def on_net_inbound(self, handler: Handler) -> Self:
        return self._scoped_on("on_net_inbound", handler)

    def on_socket_connect(self, handler: Handler) -> Self:
        return self._scoped_on("on_socket_connect", handler)

    def on_dns_query(self, handler: Handler) -> Self:
        return self._scoped_on("on_dns_query", handler)

    def on_socket_bind(self, handler: Handler) -> Self:
        return self._scoped_on("on_socket_bind", handler)

    def on_console_input(self, handler: Handler) -> Self:
        return self._scoped_on("on_console_input", handler)

    def on_console_output(self, handler: Handler) -> Self:
        return self._scoped_on("on_console_output", handler)

    def on_exec_stdout(self, handler: Handler) -> Self:
        return self._scoped_on("on_exec_stdout", handler)

    def on_exec_stderr(self, handler: Handler) -> Self:
        return self._scoped_on("on_exec_stderr", handler)

    def on_exec_stdin(self, handler: Handler) -> Self:
        return self._scoped_on("on_exec_stdin", handler)

    def on_host_lsm_exec(self, handler: Handler) -> Self:
        return self._scoped_on("on_host_lsm_exec", handler)

    def on_host_pressure(self, handler: Handler) -> Self:
        return self._scoped_on("on_host_pressure", handler)

    def on_host_keyring_access(self, handler: Handler) -> Self:
        return self._scoped_on("on_host_keyring_access", handler)

    def on_host_containment_breach(self, handler: Handler) -> Self:
        return self._scoped_on("on_host_containment_breach", handler)

    def on_vm_init(self, handler: Handler) -> Self:
        return self._scoped_on("on_vm_init", handler)

    def on_vm_ready(self, handler: Handler) -> Self:
        return self._scoped_on("on_vm_ready", handler)

    def on_vm_shutdown(self, handler: Handler) -> Self:
        return self._scoped_on("on_vm_shutdown", handler)

    def on_pre_commit(self, handler: Handler) -> Self:
        return self._scoped_on("on_pre_commit", handler)

    def on_post_commit(self, handler: Handler) -> Self:
        return self._scoped_on("on_post_commit", handler)

    def on_host_preempt(self, handler: Handler) -> Self:
        return self._scoped_on("on_host_preempt", handler)

    def on_memory_pressure(self, handler: Handler) -> Self:
        return self._scoped_on("on_memory_pressure", handler)

    def on_heartbeat(self, handler: Handler) -> Self:
        return self._scoped_on("on_heartbeat", handler)

    def on_watchdog(self, handler: Handler) -> Self:
        return self._scoped_on("on_watchdog", handler)


# see: cans/03-sdk-architecture.md#Unified-builder-surface
def _wrap_trap_deny(handler: Handler) -> Handler:
    async def run(*args: Any, **kwargs: Any) -> Any:
        try:
            result = handler(*args, **kwargs)
            if inspect.isawaitable(result):
                result = await result
            return result
        except EConfig:
            raise
        except Exception as exc:
            raise EConfig(f"on_trap_deny callback failed: {exc}") from exc

    return run
