# see: cans/02-plugin-model.md#KelirPlugin-trait
# see: cans/03-sdk-architecture.md#Prerequisite-gating-across-languages
from __future__ import annotations

import copy
import inspect
from inspect import Parameter
from pathlib import Path
from typing import TYPE_CHECKING, Any, Callable, Protocol, Self

from kelir.generated.fusion_tables import RingEvent
from kelir.plugin_fusion import HookDescriptor, HookStage, LifecycleTag
from kelir.sdk_errors import EConfig

if TYPE_CHECKING:
    from kelir.model_plugin import BasePlugin

__all__ = [
    "PluginBuilder",
    "_KelirPluginProtocol",
    "_PluginWrapper",
]

Handler = Callable[..., Any]


class _KelirPluginProtocol(Protocol):
    name: str
    requires: tuple[str, ...]
    enabled: bool

    def hook_descriptors(self) -> list[HookDescriptor]:
        ...

    def fork_state(self) -> _PluginWrapper:
        ...


# see: cans/02-plugin-model.md#KelirPlugin-trait
class _PluginWrapper:
    def __init__(
        self,
        name: str,
        requires: tuple[str, ...],
        scope: Any,
        context_fn: Handler | None,
        fork_fn: Callable[[_PluginWrapper], _PluginWrapper] | None,
        descriptors: list[HookDescriptor],
    ) -> None:
        self.name = name
        self.requires = requires
        self.enabled = True
        self.scope = scope
        self._context_fn = context_fn
        self._fork_fn = fork_fn
        self._descriptors = descriptors

    def hook_descriptors(self) -> list[HookDescriptor]:
        return list(self._descriptors)

    def fork_state(self) -> _PluginWrapper:
        if self._fork_fn is not None:
            forked = self._fork_fn(self)
            if not isinstance(forked, _PluginWrapper):
                raise EConfig(
                    f"fork_with for plugin '{self.name}' did not return a _PluginWrapper"
                )
            return forked
        return copy.copy(self)


# see: cans/02-plugin-model.md#Segregated-ring-traits
_VERB_RINGS: dict[str, tuple[str, LifecycleTag | None]] = {
    "on_trap": ("trap", None),
    "on_process_exit": ("process-exit", None),
    "on_fs_lookup": ("fs-lookup", None),
    "on_fs_read": ("fs-read", None),
    "on_fs_write": ("fs-write", None),
    "on_fs_lock": ("fs-lock", None),
    "on_net_outbound": ("net-outbound", None),
    "on_net_inbound": ("net-inbound", None),
    "on_console_input": ("console-input", None),
    "on_console_output": ("console-output", None),
    "on_exec_stdout": ("exec-stdout", None),
    "on_exec_stderr": ("exec-stderr", None),
    "on_host_lsm_exec": ("host-lsm-exec", None),
    "on_host_pressure": ("host-pressure", None),
    "on_host_keyring_access": ("host-keyring-access", None),
    "on_vm_init": ("lifecycle", "vm-init"),
    "on_vm_ready": ("lifecycle", "vm-ready"),
    "on_vm_shutdown": ("lifecycle", "vm-shutdown"),
    "on_pre_commit": ("lifecycle", "pre-commit"),
    "on_post_commit": ("lifecycle", "post-commit"),
    "on_host_preempt": ("host-preempt", None),
    "on_memory_pressure": ("memory-pressure", None),
    "on_heartbeat": ("heartbeat", None),
    "on_watchdog": ("watchdog", None),
}


# see: cans/02-plugin-model.md#KelirPlugin-trait
_VERB_ARITY: dict[str, tuple[int, ...]] = {
    "on_trap": (1, 2),
    "on_process_exit": (1, 2),
    "on_fs_lookup": (1,),
    "on_fs_read": (2,),
    "on_fs_write": (4,),
    "on_fs_lock": (4,),
    "on_net_outbound": (1,),
    "on_net_inbound": (1,),
    "on_console_input": (2,),
    "on_console_output": (2,),
    "on_exec_stdout": (3,),
    "on_exec_stderr": (3,),
    "on_host_lsm_exec": (2,),
    "on_host_pressure": (1,),
    "on_host_keyring_access": (1,),
    "on_vm_init": (1,),
    "on_vm_ready": (1,),
    "on_vm_shutdown": (1,),
    "on_pre_commit": (1,),
    "on_post_commit": (2,),
    "on_host_preempt": (1,),
    "on_memory_pressure": (2,),
    "on_heartbeat": (1,),
    "on_watchdog": (1,),
}


# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
_SCOPE_VERBS: dict[str, tuple[str, ...]] = {
    "trap": ("on_trap", "on_process_exit"),
    "fs": ("on_fs_lookup", "on_fs_read", "on_fs_write", "on_fs_lock"),
    "net": ("on_net_outbound", "on_net_inbound"),
    "console": ("on_console_input", "on_console_output"),
    "exec": ("on_exec_stdout", "on_exec_stderr"),
    "host": (
        "on_host_lsm_exec",
        "on_host_pressure",
        "on_host_keyring_access",
        "on_host_preempt",
    ),
    "lifecycle": (
        "on_vm_init",
        "on_vm_ready",
        "on_vm_shutdown",
        "on_pre_commit",
        "on_post_commit",
    ),
    "telemetry": ("on_memory_pressure", "on_heartbeat", "on_watchdog"),
}


# see: cans/artifacts/code/plugin-trait.rs#MountScope
_MOUNT_SCOPE_KEYS: frozenset[str] = frozenset(
    {"devices", "binaries", "paths", "stream_kinds"}
)


def _ring_for_value(value: str) -> RingEvent | None:
    for ring in RingEvent:
        if ring.value == value:
            return ring
    return None


def _check_arity(plugin_name: str, verb: str, handler: Handler) -> None:
    try:
        signature = inspect.signature(handler)
    except (TypeError, ValueError):
        return
    params = list(signature.parameters.values())
    if any(param.kind is Parameter.VAR_POSITIONAL for param in params):
        return
    positional = [
        param
        for param in params
        if param.kind
        in (Parameter.POSITIONAL_ONLY, Parameter.POSITIONAL_OR_KEYWORD)
    ]
    required = sum(
        1 for param in positional if param.default is Parameter.empty
    )
    total = len(positional)
    if any(required <= arity <= total for arity in _VERB_ARITY[verb]):
        return
    raise EConfig(
        f".{verb} for plugin '{plugin_name}' takes {total} positional "
        f"arguments, expected {_VERB_ARITY[verb]}"
    )


def _vsock_port(spec: Any) -> int | None:
    if isinstance(spec, dict):
        raw: Any = spec.get("port", spec.get("kind"))
    else:
        raw = getattr(spec, "port", getattr(spec, "kind", None))
    if isinstance(raw, bool) or not isinstance(raw, int):
        return None
    return raw


def _vsock_service(spec: Any) -> str | None:
    if isinstance(spec, dict):
        raw: Any = spec.get("service")
    else:
        raw = getattr(spec, "service", None)
    if not isinstance(raw, str) or not raw:
        return None
    return raw


# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
class _Scope:
    def __init__(self, builder: PluginBuilder, verbs: tuple[str, ...]) -> None:
        self._builder = builder
        self._verbs = frozenset(verbs)

    def _scoped_on(self, verb: str, handler: Handler) -> Self:
        if verb not in self._verbs:
            raise EConfig(f".{verb} is not in this scope")
        self._builder._on(verb, handler)
        return self

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


# see: cans/02-plugin-model.md#KelirPlugin-trait
class PluginBuilder:
    def __init__(self, name: str) -> None:
        self._name = name
        self._requires: list[str] = []
        self._scope: Any = None
        self._context_fn: Handler | None = None
        self._fork_fn: Callable[[_PluginWrapper], _PluginWrapper] | None = None
        self._handlers: list[tuple[str, Handler, int]] = []
        self._next_ordinal: int = 0
        self._vsock: list[tuple[Any, Handler]] = []
        self._built = False

    def _check_open(self, verb: str) -> None:
        if self._built:
            raise EConfig(
                f"PluginBuilder '{self._name}' is already built: .{verb} after .build()"
            )

    def _on(self, verb: str, handler: Handler) -> Self:
        self._check_open(verb)
        _check_arity(self._name, verb, handler)
        self._handlers.append((verb, handler, self._next_ordinal))
        self._next_ordinal += 1
        return self

    def _scope_entry(self, name: str, fn: Callable[[_Scope], Any]) -> Self:
        self._check_open(name)
        fn(_Scope(self, _SCOPE_VERBS[name]))
        return self

    def trap(self, fn: Callable[[_Scope], Any]) -> Self:
        return self._scope_entry("trap", fn)

    def fs(self, fn: Callable[[_Scope], Any]) -> Self:
        return self._scope_entry("fs", fn)

    def net(self, fn: Callable[[_Scope], Any]) -> Self:
        return self._scope_entry("net", fn)

    def console(self, fn: Callable[[_Scope], Any]) -> Self:
        return self._scope_entry("console", fn)

    def exec(self, fn: Callable[[_Scope], Any]) -> Self:
        return self._scope_entry("exec", fn)

    def host(self, fn: Callable[[_Scope], Any]) -> Self:
        return self._scope_entry("host", fn)

    def lifecycle(self, fn: Callable[[_Scope], Any]) -> Self:
        return self._scope_entry("lifecycle", fn)

    def telemetry(self, fn: Callable[[_Scope], Any]) -> Self:
        return self._scope_entry("telemetry", fn)

    def context(self, fn: Handler) -> Self:
        self._check_open("context")
        self._context_fn = fn
        return self

    def require(self, plugin: type[BasePlugin] | str) -> Self:
        self._check_open("require")
        if isinstance(plugin, str):
            required = plugin
        else:
            required = str(getattr(plugin, "name", "") or plugin.__name__)
        if not required:
            raise EConfig(f"PluginBuilder '{self._name}' .require got an unnamed plugin")
        if required not in self._requires:
            self._requires.append(required)
        return self

    def scope(self, scope: Any) -> Self:
        self._check_open("scope")
        self._scope = scope
        return self

    def fork_with(self, fn: Callable[[_PluginWrapper], _PluginWrapper]) -> Self:
        self._check_open("fork_with")
        self._fork_fn = fn
        return self

    def on_trap(self, handler: Handler) -> Self:
        return self._on("on_trap", handler)

    def on_process_exit(self, handler: Handler) -> Self:
        return self._on("on_process_exit", handler)

    def on_fs_lookup(self, handler: Handler) -> Self:
        return self._on("on_fs_lookup", handler)

    def on_fs_read(self, handler: Handler) -> Self:
        return self._on("on_fs_read", handler)

    def on_fs_write(self, handler: Handler) -> Self:
        return self._on("on_fs_write", handler)

    def on_fs_lock(self, handler: Handler) -> Self:
        return self._on("on_fs_lock", handler)

    def on_net_outbound(self, handler: Handler) -> Self:
        return self._on("on_net_outbound", handler)

    def on_net_inbound(self, handler: Handler) -> Self:
        return self._on("on_net_inbound", handler)

    def on_console_input(self, handler: Handler) -> Self:
        return self._on("on_console_input", handler)

    def on_console_output(self, handler: Handler) -> Self:
        return self._on("on_console_output", handler)

    def on_exec_stdout(self, handler: Handler) -> Self:
        return self._on("on_exec_stdout", handler)

    def on_exec_stderr(self, handler: Handler) -> Self:
        return self._on("on_exec_stderr", handler)

    def on_host_lsm_exec(self, handler: Handler) -> Self:
        return self._on("on_host_lsm_exec", handler)

    def on_host_pressure(self, handler: Handler) -> Self:
        return self._on("on_host_pressure", handler)

    def on_host_keyring_access(self, handler: Handler) -> Self:
        return self._on("on_host_keyring_access", handler)

    def on_vm_init(self, handler: Handler) -> Self:
        return self._on("on_vm_init", handler)

    def on_vm_ready(self, handler: Handler) -> Self:
        return self._on("on_vm_ready", handler)

    def on_vm_shutdown(self, handler: Handler) -> Self:
        return self._on("on_vm_shutdown", handler)

    def on_pre_commit(self, handler: Handler) -> Self:
        return self._on("on_pre_commit", handler)

    def on_post_commit(self, handler: Handler) -> Self:
        return self._on("on_post_commit", handler)

    def on_host_preempt(self, handler: Handler) -> Self:
        return self._on("on_host_preempt", handler)

    def on_memory_pressure(self, handler: Handler) -> Self:
        return self._on("on_memory_pressure", handler)

    def on_heartbeat(self, handler: Handler) -> Self:
        return self._on("on_heartbeat", handler)

    def on_watchdog(self, handler: Handler) -> Self:
        return self._on("on_watchdog", handler)

    def vsock_service(self, spec: Any, handler: Handler) -> Self:
        self._check_open("vsock_service")
        self._vsock.append((spec, handler))
        return self

    def _validate_scope(self) -> None:
        scope = self._scope
        if scope is None:
            return
        if not isinstance(scope, dict):
            raise EConfig(
                f"scope for plugin '{self._name}' must be a MountScope mapping, "
                f"got {type(scope).__name__}"
            )
        unknown = sorted(set(scope) - _MOUNT_SCOPE_KEYS)
        if unknown:
            raise EConfig(
                f"scope for plugin '{self._name}' has unknown MountScope keys: {unknown}"
            )
        for key, value in scope.items():
            if not isinstance(value, (list, tuple)):
                raise EConfig(
                    f"scope for plugin '{self._name}' key '{key}' must be a list"
                )
            for entry in value:
                if isinstance(entry, bool) or not isinstance(entry, (str, Path, int)):
                    raise EConfig(
                        f"scope for plugin '{self._name}' key '{key}' has an "
                        f"invalid entry {entry!r}"
                    )

    def _descriptors(self) -> list[HookDescriptor]:
        descriptors: list[HookDescriptor] = []
        for verb, handler, ordinal in self._handlers:
            ring_value, lifecycle_tag = _VERB_RINGS[verb]
            ring = _ring_for_value(ring_value)
            if ring is None:
                raise EConfig(
                    f"ring '{ring_value}' for .{verb} has no Python RingEvent yet"
                )
            descriptors.append(
                HookDescriptor(
                    ring=ring,
                    stage=HookStage.SINGLE,
                    ordinal=ordinal,
                    plugin_name=self._name,
                    lifecycle_tag=lifecycle_tag,
                    run=handler,
                )
            )
        ordinal = self._next_ordinal
        for spec, handler in self._vsock:
            port = _vsock_port(spec)
            if port is None or not 1 <= port <= 65535:
                raise EConfig(
                    f"vsock service for plugin '{self._name}' has invalid port "
                    f"{port!r} (expected 1..65535)"
                )
            if _vsock_service(spec) is None:
                raise EConfig(
                    f"vsock service for plugin '{self._name}' declares no service name"
                )
            descriptors.append(
                HookDescriptor(
                    ring=RingEvent.VSOCK_SERVICE,
                    stage=HookStage.SINGLE,
                    ordinal=ordinal,
                    plugin_name=self._name,
                    vsock_kind=port,
                    run=handler,
                )
            )
            ordinal += 1
        return sorted(descriptors, key=lambda hook: hook.ordinal)

    def build(self) -> _PluginWrapper:
        self._check_open("build")
        self._validate_scope()
        wrapper = _PluginWrapper(
            name=self._name,
            requires=tuple(self._requires),
            scope=self._scope,
            context_fn=self._context_fn,
            fork_fn=self._fork_fn,
            descriptors=self._descriptors(),
        )
        self._built = True
        return wrapper
