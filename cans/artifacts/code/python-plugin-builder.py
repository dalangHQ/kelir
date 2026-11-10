# ILLUSTRATIVE, NON-STANDALONE — do not import this file directly.
# It pins the Python fluent plugin-creation builder surface; the live shape
# is packages/py/kelir/plugin_builder.py (Agent 5) and the normative
# contract is cans/02-plugin-model.md#Fluent-plugin-creation-builder.
#
# Owned by: cans/02-plugin-model.md#Fluent-plugin-creation-builder
# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
# see: cans/03-sdk-architecture.md#Fluent-plugin-surface

from __future__ import annotations

from typing import Any, Callable

from kelir.sdk_errors import EConfig

# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
class PluginBuilder:
    def __init__(self, name: str) -> None:
        self._name = name
        self._requires: list[str] = []
        self._scope: dict[str, Any] | None = None
        self._fork_with: Callable[[Any], Any] | None = None
        self._handlers: dict[str, Callable[..., Any]] = {}
        self._vsock: list[tuple[Any, Callable[..., Any]]] = []
        self._built: bool = False
        self._context_fn: Callable[[], Any] | None = None

    def _check_open(self) -> None:
        if self._built:
            raise EConfig(f"plugin {self._name!r} already built: post-build mutation is rejected")

    def context(self, fn: Callable[[], Any]) -> PluginBuilder:
        self._check_open()
        self._context_fn = fn
        return self

    def require(self, plugin_type_or_name: str | type) -> PluginBuilder:
        self._check_open()
        name = plugin_type_or_name if isinstance(plugin_type_or_name, str) else plugin_type_or_name.__name__
        self._requires.append(name)
        return self

    def scope(self, scope: dict[str, Any]) -> PluginBuilder:
        self._check_open()
        self._scope = scope
        return self

    def fork_with(self, fn: Callable[[Any], Any]) -> PluginBuilder:
        self._check_open()
        self._fork_with = fn
        return self

    def on_trap(self, handler: Callable[..., Any]) -> PluginBuilder:
        self._check_open()
        self._handlers["trap"] = handler
        return self

    def on_fs_write(self, handler: Callable[..., Any]) -> PluginBuilder:
        self._check_open()
        self._handlers["fs-write"] = handler
        return self

    def on_net_outbound(self, handler: Callable[..., Any]) -> PluginBuilder:
        self._check_open()
        self._handlers["net-outbound"] = handler
        return self

    def vsock_service(self, spec: Any, handler: Callable[..., Any]) -> PluginBuilder:
        self._check_open()
        self._vsock.append((spec, handler))
        return self

    def build(self) -> _PluginWrapper:
        self._check_open()
        self._built = True
        return _PluginWrapper(self)

# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
class _PluginWrapper:
    def __init__(self, builder: PluginBuilder) -> None:
        self._builder = builder
        self.name = builder._name

    def fork_state(self, child_session_id: str) -> _PluginWrapper:
        # Re-runs the caller's .fork_with closure when provided; otherwise
        # the wrapper is re-initialized (state reset).
        return _PluginWrapper(self._builder)

# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
def build_chaos_plugin() -> _PluginWrapper:
    return (
        PluginBuilder("chaos")
        .require("ledger")
        .scope({"paths": ["/workspace"]})
        .on_trap(lambda event, ctx: "Pass")
        .on_fs_write(lambda path, data, pid, ctx: "Forward")
        .build()
    )

# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
# EConfig triggers pinned by the contract:
#   - .require(P) naming a plugin absent from the builder's
#     _plugin_registrations raises EConfig at builder.boot(), listing the
#     unsatisfied names (checked in packages/py/kelir/__init__.py:1022).
#   - Any verb call after .build() raises EConfig (post-build mutation).
#   - .build() before .use(): KelirBuilder.use() accepts the _PluginWrapper
#     (post-build) only; passing a pre-build PluginBuilder raises EConfig.
# The remaining verbs (.on_process_exit, .on_fs_lookup, .on_fs_read,
# .on_fs_lock, .on_net_inbound, .on_console_input, .on_console_output,
# .on_exec_stdout, .on_exec_stderr, .on_host_lsm_exec, .on_host_pressure,
# .on_host_keyring_access, .on_vm_init, .on_vm_ready, .on_vm_shutdown,
# .on_pre_commit, .on_post_commit, .on_host_preempt, .on_memory_pressure,
# .on_heartbeat, .on_watchdog) follow the .on_trap shape above: one handler
# registered under its ring tag, bound by the wrapper at .use() time.
