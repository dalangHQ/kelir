# see: cans/03-sdk-architecture.md#Python-SDK
# see: cans/02-plugin-model.md#Foreign-plugin-proxy
from __future__ import annotations

import inspect
from dataclasses import dataclass
from inspect import Parameter
from pathlib import Path
from types import TracebackType
from typing import TYPE_CHECKING, Any, Awaitable, Callable, Literal, NoReturn, Self

from kelir.builder_scopes import _VERB_ARITY, _VERB_RINGS
from kelir.service_caps import materialize_cap
from kelir.plugin_fusion import (
    FusedTrampoline,
    HookDescriptor,
    HookStage,
    PluginRegistration,
    RingEvent,
    TrapFilterSnapshot,
)
from kelir.sdk_errors import EConfig

if TYPE_CHECKING:
    from kelir.model_plugin import BasePlugin

from kelir import Caps, Session, TrapFilter


@dataclass
class VsockServiceEntry:
    port: int
    service: str
    schema: dict[str, Any]
    handler: Callable[[bytes], Awaitable[bytes]] | None = None


Handler = Callable[..., Any]




# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
_SCOPE_VERBS: dict[str, tuple[str, ...]] = {
    "trap": ("on_trap", "on_process_exit", "on_trap_deny"),
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
    def __init__(self, builder: KelirBuilder, verbs: tuple[str, ...]) -> None:
        self._builder = builder
        self._verbs = frozenset(verbs)

    def _scoped_on(self, verb: str, handler: Handler) -> Self:
        if verb not in self._verbs:
            raise EConfig(f".{verb} is not in this scope")
        self._builder._on(verb, handler)
        return self

    def on_trap_deny(self, handler: Handler) -> Self:
        if "on_trap_deny" not in self._verbs:
            raise EConfig(".on_trap_deny is not in this scope")
        self._builder.on_trap_deny(handler)
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


# see: cans/02-plugin-model.md#Unified-builder
def _plugin_create(name: str) -> KelirBuilder:
    if not isinstance(name, str) or not name:
        raise EConfig("KelirBuilder.plugin(name) expects a non-empty plugin name")
    builder = KelirBuilder()
    builder._mode = "plugin"
    builder._name = name
    return builder


# see: cans/02-plugin-model.md#Unified-builder
class _PluginMount:
    def __get__(
        self, obj: KelirBuilder | None, objtype: type[KelirBuilder] | None = None
    ) -> Callable[..., KelirBuilder]:
        if obj is None:
            return _plugin_create
        return _plugin_instance_mount(obj)


def _plugin_instance_mount(host: KelirBuilder) -> Callable[..., KelirBuilder]:
    def mount(plugin: Any) -> KelirBuilder:
        if isinstance(plugin, str):
            raise EConfig(
                ".plugin() on an instance mounts a plugin-mode builder, "
                f"not a name: got {plugin!r}; create it with "
                f"KelirBuilder.plugin({plugin!r}) then mount it with "
                ".use(plugin) or .plugin(builder)"
            )
        if not isinstance(plugin, KelirBuilder):
            raise EConfig(
                ".plugin() on an instance mounts only a plugin-mode "
                "KelirBuilder; mount a BasePlugin with .use(plugin)"
            )
        return host.use(plugin)

    return mount


class KelirBuilder:
    def __init__(
        self,
        *,
        vcpus: int | None = None,
        memory_mb: int | None = None,
        page_size_kb: int | None = None,
        deterministic_clock: bool | None = None,
        pinned_clocksource: str | None = None,
        preemption_grace_ms: int | None = None,
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
        self._next_ordinal: int = 0
        self._session: Session | None = None
        self._params_model: type | None = None
        self._config: dict[str, Any] = {
            "vcpus": vcpus,
            "memory_mb": memory_mb,
            "page_size_kb": page_size_kb,
            "deterministic_clock": deterministic_clock,
            "pinned_clocksource": pinned_clocksource,
            "preemption_grace_ms": preemption_grace_ms,
            "kernel": kernel,
            "rootfs_erofs": rootfs_erofs,
            "tsc_locked": tsc_locked,
            "enable_watchdog": enable_watchdog,
            "driver": driver,
            "detached": detached,
        }
        self._mode: Literal["host", "plugin"] = "host"
        self._name: str | None = None
        self._requires: list[str] = []
        self._scope: Any = None
        self._context_fn: Handler | None = None
        self._fork_fn: Callable[[KelirBuilder], KelirBuilder] | None = None
        self._handlers: list[tuple[str, Handler, int]] = []
        # TODO (L3) [plugin_engine]: the engine dispatch reads these entries once register_interceptors wiring lands per cans/02-plugin-model.md#Static-dispatch.; slop: interceptors never registered, traps bypass filter chain
        self._vsock: list[tuple[Any, Handler]] = []
        self._mounted: bool = False
        self._mounted_on: list[KelirBuilder] = []
        self.enabled: bool = True

    # see: cans/02-plugin-model.md#Unified-builder
    plugin = _PluginMount()

    @property
    def name(self) -> str | None:
        return self._name

    @property
    def requires(self) -> tuple[str, ...]:
        return tuple(self._requires)

    # see: cans/08-lifecycle.md#Detached-lifecycle
    def detached(self, detached: bool) -> Self:
        if self._mode == "plugin":
            self._raise_host_only("detached")
        self._config["detached"] = detached
        return self

    def is_detached(self) -> bool:
        return bool(self._config.get("detached", False))

    # see: cans/04-terminal-plane.md#Terminal-pipeline
    def terminal(self, **kwargs: Any) -> Self:
        if self._mode == "plugin":
            self._raise_host_only("terminal")
        self._config["terminal"] = kwargs
        return self

    # see: cans/05-storage-plane.md#Workspace-commit
    def storage(self, **kwargs: Any) -> Self:
        if self._mode == "plugin":
            self._raise_host_only("storage")
        self._config.setdefault("storage", {}).update(kwargs)
        return self

    # see: cans/05-storage-plane.md#Workspace-commit
    def lock_broker(self, enabled: bool = True) -> Self:
        if self._mode == "plugin":
            self._raise_host_only("lock_broker")
        if not isinstance(enabled, bool):
            raise EConfig(".lock_broker(enabled) expects a bool")
        self._config.setdefault("storage", {})["lock_broker"] = enabled
        return self

    # see: cans/07-wire-plane.md#DNS-policy-filter
    def wire(self, **kwargs: Any) -> Self:
        if self._mode == "plugin":
            self._raise_host_only("wire")
        self._config["wire"] = kwargs
        return self

    # see: cans/03-sdk-architecture.md#Python-SDK
    def params(self, model: type) -> Self:
        if self._mode == "plugin":
            self._raise_host_only("params")
        # TODO (L3) [bp_01_python_boot_econfig_missing_pydantic_dependency]: validated params models ride the native boot once the PyO3 binding awaits.; slop: boot passes without runtime pydantic validation
        self._params_model = model
        return self

    # see: cans/02-plugin-model.md#KelirPlugin-trait
    def use(self, plugin: BasePlugin[Any, Any] | KelirBuilder) -> Self:
        from kelir.model_plugin import BasePlugin as _BasePlugin
        from kelir.model_plugin import hook_descriptors

        if isinstance(plugin, KelirBuilder):
            if plugin._mode != "plugin":
                raise EConfig("only a plugin-mode KelirBuilder can be mounted with .use()")
            for descriptor in plugin.hook_descriptors():
                descriptor.ordinal = self._next_ordinal
                self._next_ordinal += 1
                self._hooks.append(descriptor)
            self._plugins.append(plugin)
            self._plugin_registrations.append(
                PluginRegistration(name=str(plugin._name), enabled=plugin.enabled)
            )
            plugin._mounted = True
            plugin._mounted_on.append(self)
            return self
        if not isinstance(plugin, _BasePlugin):
            raise EConfig("expected a BasePlugin or a plugin-mode KelirBuilder")
        name = getattr(plugin, "name", None) or type(plugin).__name__
        enabled = bool(getattr(plugin, "enabled", True))
        self._plugins.append(plugin)
        self._plugin_registrations.append(
            PluginRegistration(name=str(name), enabled=enabled)
        )
        if enabled:
            for descriptor in hook_descriptors(plugin):
                descriptor.ordinal = self._next_ordinal
                self._next_ordinal += 1
                self._hooks.append(descriptor)
        return self

    def trap(
        self,
        pattern: str | TrapFilter | Callable[[_Scope], Any],
        pre: Callable[..., Awaitable[Any]] | None = None,
        post: Callable[..., Awaitable[Any]] | None = None,
    ) -> Self:
        # see: cans/02-plugin-model.md#Foreign-plugin-proxy
        self._check_mounted("trap")
        if callable(pattern):
            return self._scope_entry("trap", pattern)
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
        if self._mode == "plugin":
            self._raise_host_only("register_custom_action")
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
    def register_stream_kind(
        self,
        kind: int,
        schema: dict[str, Any],
        handler: Callable[[bytes], Awaitable[bytes]],
    ) -> Self:
        # see: cans/02-plugin-model.md#Foreign-plugin-proxy
        if self._mode == "plugin":
            self._raise_host_only("register_stream_kind")
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

    # see: cans/02-plugin-model.md#Unified-builder
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

    def on_trap_deny(self, handler: Handler) -> Self:
        if self._mode == "plugin":
            self._raise_host_only("on_trap_deny")
        self._hooks.append(
            HookDescriptor(
                ring=RingEvent.LIFECYCLE,
                stage=HookStage.SINGLE,
                ordinal=self._next_ordinal,
                lifecycle_tag="trap-deny",
                run=handler,
            )
        )
        self._next_ordinal += 1
        return self

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

    # see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
    def group(self, fn: Callable[[KelirBuilder], KelirBuilder | None]) -> Self:
        self._check_mounted("group")
        result = fn(self)
        if result is not None and result is not self:
            raise EConfig("group() must return the builder it receives")
        return self

    # see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
    def guard(self, enabled: bool, fn: Callable[[KelirBuilder], KelirBuilder | None]) -> Self:
        self._check_mounted("guard")
        if not isinstance(enabled, bool):
            raise EConfig(".guard(enabled, ...) expects a bool")
        if enabled:
            return self.group(fn)
        return self

    # see: cans/02-plugin-model.md#Unified-builder
    def context(self, fn: Handler) -> Self:
        self._check_mounted("context")
        if self._mode == "host":
            raise EConfig(
                ".context() is plugin-only: a host-mode KelirBuilder has no "
                "plugin context; call it on KelirBuilder.plugin(name)"
            )
        self._context_fn = fn
        return self

    def require(self, plugin: type | str) -> Self:
        self._check_mounted("require")
        if isinstance(plugin, str):
            required = plugin
        else:
            required = str(getattr(plugin, "name", "") or plugin.__name__)
        if not required:
            raise EConfig(
                f".require for plugin '{self._name or 'host'}' got an unnamed plugin"
            )
        if required not in self._requires:
            self._requires.append(required)
        return self

    def scope(self, scope: Any) -> Self:
        self._check_mounted("scope")
        self._scope = scope
        return self

    def fork_with(self, fn: Callable[[KelirBuilder], KelirBuilder]) -> Self:
        self._check_mounted("fork_with")
        self._fork_fn = fn
        return self

    def fork_state(self) -> KelirBuilder:
        if self._mode == "host":
            raise EConfig(
                "host-mode KelirBuilder forks via Session.fork(), not .fork_state()"
            )
        if self._fork_fn is not None:
            forked = self._fork_fn(self)
            if not isinstance(forked, KelirBuilder) or forked._mode != "plugin":
                raise EConfig(
                    f"fork_with for plugin '{self._name}' did not return "
                    "a plugin-mode KelirBuilder"
                )
            return forked
        forked = KelirBuilder.plugin(self._name or "plugin")
        forked._requires = list(self._requires)
        forked._scope = self._scope
        forked._context_fn = self._context_fn
        forked._handlers = list(self._handlers)
        forked._vsock = list(self._vsock)
        forked._next_ordinal = self._next_ordinal
        return forked

    def vsock_service(self, spec: Any, handler: Handler | None = None) -> Self:
        self._check_mounted("vsock_service")
        entry_handler: Handler | None = None
        if isinstance(spec, VsockServiceEntry):
            entry_handler = spec.handler
        if handler is not None and entry_handler is not None:
            raise EConfig(
                "vsock service handler passed twice: pass it once, "
                "as the vsock_service() argument"
            )
        resolved = handler if handler is not None else entry_handler
        if resolved is None:
            raise EConfig("vsock service declares no handler")
        raw: Any = spec.get("port") if isinstance(spec, dict) else getattr(spec, "port", None)
        if raw is not None and (
            isinstance(raw, bool) or not isinstance(raw, int) or raw < 0 or raw > 0xFFFFFFFF
        ):
            raise EConfig(f"vsock service port {raw} outside u32")
        self._vsock.append((spec, resolved))
        return self

    def _on(self, verb: str, handler: Handler) -> Self:
        self._check_mounted(verb)
        _check_arity(self._name or "host", verb, handler)
        ring_value, lifecycle_tag = _VERB_RINGS[verb]
        ring = _ring_for_value(ring_value)
        if ring is None:
            raise EConfig(f"ring '{ring_value}' for .{verb} has no Python RingEvent yet")
        if self._mode == "plugin":
            self._handlers.append((verb, handler, self._next_ordinal))
        else:
            self._hooks.append(
                HookDescriptor(
                    ring=ring,
                    stage=HookStage.SINGLE,
                    ordinal=self._next_ordinal,
                    lifecycle_tag=lifecycle_tag,
                    run=handler,
                )
            )
        self._next_ordinal += 1
        return self

    def _scope_entry(self, name: str, fn: Callable[[_Scope], Any]) -> Self:
        self._check_mounted(name)
        fn(_Scope(self, _SCOPE_VERBS[name]))
        return self

    def _check_mounted(self, verb: str) -> None:
        if self._mode == "plugin" and self._mounted:
            raise EConfig(f"plugin already mounted: {self._name}")

    def _raise_host_only(self, verb: str) -> NoReturn:
        raise EConfig(
            f"{self._name}: host-only verb .{verb}() called on plugin-mode builder"
        )

    # see: cans/02-plugin-model.md#Unified-builder
    def hook_descriptors(self) -> list[HookDescriptor]:
        if self._mode == "host":
            return list(self._hooks)
        return self._build_descriptors()

    def _build_descriptors(self) -> list[HookDescriptor]:
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
        ordinal = len(self._handlers)
        for spec, handler in self._vsock:
            descriptors.append(
                HookDescriptor(
                    ring=RingEvent.VSOCK_SERVICE,
                    stage=HookStage.SINGLE,
                    ordinal=ordinal,
                    plugin_name=self._name,
                    vsock_kind=_vsock_port(spec),
                    run=handler,
                )
            )
            ordinal += 1
        descriptors.extend(self._hooks)
        return sorted(descriptors, key=lambda hook: hook.ordinal)

    def _validate_scope(self, name: str | None, scope: Any) -> None:
        if scope is None:
            return
        label = name or "host"
        if not isinstance(scope, dict):
            raise EConfig(
                f"scope for plugin '{label}' must be a MountScope mapping, "
                f"got {type(scope).__name__}"
            )
        unknown = sorted(set(scope) - _MOUNT_SCOPE_KEYS)
        if unknown:
            raise EConfig(
                f"scope for plugin '{label}' has unknown MountScope keys: {unknown}"
            )
        for key, value in scope.items():
            if not isinstance(value, (list, tuple)):
                raise EConfig(
                    f"scope for plugin '{label}' key '{key}' must be a list"
                )
            for entry in value:
                if isinstance(entry, bool) or not isinstance(entry, (str, Path, int)):
                    raise EConfig(
                        f"scope for plugin '{label}' key '{key}' has an "
                        f"invalid entry {entry!r}"
                    )

    def _validate_vsock(self, name: str | None, vsock: list[tuple[Any, Handler]]) -> None:
        label = name or "host"
        for spec, _handler in vsock:
            port = _vsock_port(spec)
            if port is None or not 1 <= port <= 65535:
                raise EConfig(
                    f"vsock service for plugin '{label}' has invalid port "
                    f"{port!r} (expected 1..65535)"
                )
            if _vsock_service(spec) is None:
                raise EConfig(
                    f"vsock service for plugin '{label}' declares no service name"
                )

    def _validate_plugin_shape(self, plugin: KelirBuilder) -> None:
        self._validate_scope(plugin._name, plugin._scope)
        self._validate_vsock(plugin._name, plugin._vsock)

    def _validate_all(self) -> None:
        seen: set[str] = set()
        for registration in self._plugin_registrations:
            if registration.name in seen:
                raise EConfig(f"plugin already mounted: {registration.name}")
            seen.add(registration.name)
        registered = {registration.name for registration in self._plugin_registrations}
        for required in self._requires:
            if required not in registered:
                raise EConfig(f"missing prerequisite: {required} required by host")
        for plugin in self._plugins:
            if isinstance(plugin, KelirBuilder):
                if any(host is not self for host in plugin._mounted_on):
                    raise EConfig(f"plugin already mounted: {plugin._name}")
                self._validate_plugin_shape(plugin)
                for required in plugin._requires:
                    if required not in registered:
                        raise EConfig(
                            f"missing prerequisite: {required} required by {plugin._name}"
                        )
            else:
                name = getattr(plugin, "name", None) or type(plugin).__name__
                required_names = getattr(plugin, "requires", ())
                if isinstance(required_names, str):
                    required_names = (required_names,)
                for required in required_names:
                    if required not in registered:
                        raise EConfig(
                            f"missing prerequisite: {required} required by {name}"
                        )
        self._validate_scope(self._name, self._scope)
        self._validate_vsock(self._name, self._vsock)

    # see: cans/02-plugin-model.md#Unified-builder
    def seal(self) -> Self:
        self._validate_all()
        return self

    @property
    def vsock_services(self) -> list[VsockServiceEntry]:
        return [spec for spec, _ in self._vsock if isinstance(spec, VsockServiceEntry)]

    # see: cans/02-plugin-model.md#Foreign-plugin-proxy
    def fuse(self) -> FusedTrampoline:
        return FusedTrampoline.fuse(self.hook_descriptors())

    async def boot(self) -> Session:
        # see: cans/03-sdk-architecture.md#Python-SDK
        from kelir import _bridge_safe_config, _load_native_bridge
        if self._mode == "plugin":
            raise EConfig(
                "cannot boot a plugin-mode KelirBuilder; mount it on a host with .use()"
            )
        self.seal()
        fused = self.fuse()
        descriptor = fused.describe()
        caps = Caps()
        bridge = _load_native_bridge()
        native = bridge.PyKelirBuilder(
            vcpus=self._config["vcpus"],
            memory_mb=self._config["memory_mb"],
            kernel=self._config["kernel"],
            rootfs_erofs=self._config["rootfs_erofs"],
            tsc_locked=self._config["tsc_locked"],
            enable_watchdog=self._config["enable_watchdog"],
            driver=self._config["driver"],
        )
        # TODO (L3) [bindings_py]: forward page_size_kb/deterministic_clock/pinned_clocksource/preemption_grace_ms once the PyO3 constructor accepts them.; slop: boot flags silently dropped by native constructor
        if "terminal" in self._config:
            # TODO (L3) [bindings_py]: token_shield and prompt callbacks stay Python-side until the FFI callback bridge lands.; slop: token shield/prompt callbacks never cross to native
            native.set_terminal_config(
                _bridge_safe_config("terminal", self._config["terminal"])
            )
        if "storage" in self._config:
            # TODO (L3) [bindings_py]: snoop on_write callbacks stay Python-side until the FFI callback bridge lands.; slop: snoop on_write never fires on guest writes
            native.set_storage_config(
                _bridge_safe_config("storage", self._config["storage"])
            )
        if "wire" in self._config:
            native.set_wire_config(_bridge_safe_config("wire", self._config["wire"]))
        for registration in self._plugin_registrations:
            native.use_plugin(registration.name, registration.enabled)
        native.fuse_hooks(descriptor, fused.runs(caps))
        native_handle = native.boot()
        session = Session.from_native(native_handle, cap=caps)
        # see: cans/03-sdk-architecture.md#Service-lookup-across-languages
        get_plugin_context = getattr(native_handle, "get_plugin_context", None)
        if get_plugin_context is None:
            if self._plugin_registrations:
                raise EConfig(
                    "kelir native PyO3 bridge (_kelir) is missing: session handle get_plugin_context(name) (B13)"
                )
        else:
            for registration in self._plugin_registrations:
                if not registration.enabled:
                    continue
                record = get_plugin_context(registration.name)
                if not isinstance(record, dict):
                    raise EConfig(
                        f"plugin '{registration.name}' context is not a mapping; capability struct cannot be materialized (B13)"
                    )
                materialize_cap(session.cap, registration.name, record)
        return session

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
            await self._session.terminate()
            self._session = None


def kelir() -> KelirBuilder:
    return KelirBuilder()


def builder(
    *,
    vcpus: int | None = None,
    memory_mb: int | None = None,
    page_size_kb: int | None = None,
    deterministic_clock: bool | None = None,
    pinned_clocksource: str | None = None,
    preemption_grace_ms: int | None = None,
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
        page_size_kb=page_size_kb,
        deterministic_clock=deterministic_clock,
        pinned_clocksource=pinned_clocksource,
        preemption_grace_ms=preemption_grace_ms,
        kernel=kernel,
        rootfs_erofs=rootfs_erofs,
        tsc_locked=tsc_locked,
        enable_watchdog=enable_watchdog,
        driver=driver,
        detached=detached,
    )
