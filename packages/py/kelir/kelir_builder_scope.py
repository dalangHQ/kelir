# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
from __future__ import annotations

import inspect
from typing import TYPE_CHECKING, Any, Callable, Self

from kelir.sdk_errors import EConfig

if TYPE_CHECKING:
    from kelir.kelir_builder import KelirBuilder


Handler = Callable[..., Any]

_FS_CONFIG_KEYS: frozenset[str] = frozenset(
    {"lock_broker", "carryover_bytes", "scan_engine", "snoops"}
)


# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
class _Scope:
    def __init__(self, builder: KelirBuilder, verbs: tuple[str, ...]) -> None:
        self._builder = builder
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
        self._builder.on_trap_deny(_wrap_trap_deny(handler))
        return self

    def register(
        self, name_or_kind: str | int, schema: dict[str, Any], handler: Handler
    ) -> Self:
        if "register_custom_action" in self._verbs and isinstance(name_or_kind, str):
            self._builder.register_custom_action(name_or_kind, schema, handler)
        elif "register_stream_kind" in self._verbs and isinstance(name_or_kind, int):
            self._builder.register_stream_kind(name_or_kind, schema, handler)
        else:
            raise EConfig(".register is not in this scope")
        return self

    def service(self, port: int, service: str, handler: Handler) -> Self:
        if "vsock_service" not in self._verbs:
            raise EConfig(".service is not in this scope")
        self._builder.vsock_service({"port": port, "service": service}, handler)
        return self

    def model(self, model: type) -> Self:
        if "params" not in self._verbs:
            raise EConfig(".model is not in this scope")
        self._builder.params(model)
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
        # TODO (L3) [bindings_py]: apply the stored fs config in the live lock broker, scan engine, and carryover path.; slop: stored fs config never applied, the scope stays inert.
        self._builder._fs_scope["config"] = validated
        return self

    def require(self, plugin: type | str) -> Self:
        self._builder.require(plugin)
        return self

    def fork_with(self, fn: Callable[[KelirBuilder], KelirBuilder]) -> Self:
        self._builder.fork_with(fn)
        return self

    def timeout(self, seconds: float) -> Self:
        if isinstance(seconds, bool) or not isinstance(seconds, (int, float)):
            raise EConfig(".timeout(seconds) expects a number")
        if seconds < 0:
            raise EConfig(".timeout(seconds) expects a non-negative number")
        self._builder._fs_scope["timeout"] = seconds
        return self

    def priority(self, priority: int) -> Self:
        if isinstance(priority, bool) or not isinstance(priority, int):
            raise EConfig(".priority(priority) expects an int")
        self._builder._fs_scope["priority"] = priority
        return self

    def rules(self, rules: dict[str, Any]) -> Self:
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
        # TODO (L3) [bindings_py]: apply the stored fs rules in the live read/write enforcement path.; slop: stored fs rules never enforced, reads/writes run unguarded.
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
