# see: cans/02-plugin-model.md#KelirPlugin-trait
# see: cans/03-sdk-architecture.md#Prerequisite-gating-across-languages
from __future__ import annotations

import copy
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


def _ring_for_value(value: str) -> RingEvent | None:
    for ring in RingEvent:
        if ring.value == value:
            return ring
    return None


def _vsock_kind(spec: Any) -> int | None:
    if isinstance(spec, dict):
        raw: Any = spec.get("kind", spec.get("port"))
    else:
        raw = getattr(spec, "kind", getattr(spec, "port", None))
    if isinstance(raw, bool) or not isinstance(raw, int):
        return None
    return raw


# see: cans/02-plugin-model.md#KelirPlugin-trait
class PluginBuilder:
    def __init__(self, name: str) -> None:
        self._name = name
        self._requires: list[str] = []
        self._scope: Any = None
        self._context_fn: Handler | None = None
        self._fork_fn: Callable[[_PluginWrapper], _PluginWrapper] | None = None
        self._handlers: dict[str, Handler] = {}
        self._ordinals: dict[str, int] = {}
        self._next_ordinal: int = 0
        self._vsock_spec: Any = None
        self._vsock_handler: Handler | None = None
        self._built = False

    def _check_open(self, verb: str) -> None:
        if self._built:
            raise EConfig(
                f"PluginBuilder '{self._name}' is already built: .{verb} after .build()"
            )

    def _on(self, verb: str, handler: Handler) -> Self:
        self._check_open(verb)
        if verb not in self._ordinals:
            self._ordinals[verb] = self._next_ordinal
            self._next_ordinal += 1
        self._handlers[verb] = handler
        return self

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
        self._vsock_spec = spec
        self._vsock_handler = handler
        return self

    def _descriptors(self) -> list[HookDescriptor]:
        descriptors: list[HookDescriptor] = []
        for verb in sorted(self._handlers, key=lambda v: self._ordinals[v]):
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
                    ordinal=self._ordinals[verb],
                    plugin_name=self._name,
                    lifecycle_tag=lifecycle_tag,
                    run=self._handlers[verb],
                )
            )
        if self._vsock_handler is not None:
            kind = _vsock_kind(self._vsock_spec)
            if kind is None or not 0 <= kind <= 0xFFFF:
                raise EConfig(
                    f"vsock service for plugin '{self._name}' declares no u16 stream kind"
                )
            descriptors.append(
                HookDescriptor(
                    ring=RingEvent.VSOCK_SERVICE,
                    stage=HookStage.SINGLE,
                    ordinal=self._next_ordinal,
                    plugin_name=self._name,
                    vsock_kind=kind,
                    run=self._vsock_handler,
                )
            )
        return sorted(descriptors, key=lambda hook: hook.ordinal)

    def build(self) -> _PluginWrapper:
        self._check_open("build")
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
