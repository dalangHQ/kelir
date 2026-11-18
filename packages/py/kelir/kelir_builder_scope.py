# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
from __future__ import annotations

import inspect
from typing import TYPE_CHECKING, Any, Callable, Self

from kelir.sdk_errors import EConfig

if TYPE_CHECKING:
    from kelir.kelir_builder import KelirBuilder


Handler = Callable[..., Any]


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

    def end(self) -> KelirBuilder:
        return self._builder

    def on_trap(self, handler: Handler) -> Self:
        return self._scoped_on("on_trap", handler)

    def on_process_exit(self, handler: Handler) -> Self:
        return self._scoped_on("on_process_exit", handler)

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

    def on_net_outbound(self, handler: Handler) -> Self:
        return self._scoped_on("on_net_outbound", handler)

    def on_net_inbound(self, handler: Handler) -> Self:
        return self._scoped_on("on_net_inbound", handler)

    def on_console_input(self, handler: Handler) -> Self:
        return self._scoped_on("on_console_input", handler)

    def on_console_output(self, handler: Handler) -> Self:
        return self._scoped_on("on_console_output", handler)

    def on_exec_stdout(self, handler: Handler) -> Self:
        return self._scoped_on("on_exec_stdout", handler)

    def on_exec_stderr(self, handler: Handler) -> Self:
        return self._scoped_on("on_exec_stderr", handler)

    def on_host_lsm_exec(self, handler: Handler) -> Self:
        return self._scoped_on("on_host_lsm_exec", handler)

    def on_host_pressure(self, handler: Handler) -> Self:
        return self._scoped_on("on_host_pressure", handler)

    def on_host_keyring_access(self, handler: Handler) -> Self:
        return self._scoped_on("on_host_keyring_access", handler)

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
