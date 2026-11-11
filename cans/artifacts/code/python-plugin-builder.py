# ILLUSTRATIVE, NON-STANDALONE — do not import this file directly.
# It pins the Python unified builder surface; the live shape is
# packages/py/kelir/__init__.py KelirBuilder (Agent 5) and the normative
# contract is cans/02-plugin-model.md#Unified-builder.
#
# Target shape: ONE class KelirBuilder. KelirBuilder(...) with VmConfig
# kwargs is the host; KelirBuilder.plugin(name) is a plugin-mode builder.
# The 24 ring verbs + 8 closure scopes + context / require / scope /
# fork_with / vsock_service exist in BOTH modes. Mount is merge by
# value: host.use(plugin) appends the plugin's hook_descriptors() after
# the host's own hooks. Validation happens once, in boot(), over the host
# + mounted plugins together. There is no build(): a plugin-mode builder
# is mounted as-is, and the _built flag / _check_open machinery deletes.
# PluginBuilder / _PluginWrapper / plugin_builder.py are deleted.
#
# Owned by: cans/02-plugin-model.md#Unified-builder
# see: cans/02-plugin-model.md#Unified-builder
# see: cans/03-sdk-architecture.md#Unified-builder-surface
# see: cans/artifacts/examples/fluent-builder-chaos.py#build_chaos_plugin

from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, Awaitable, Callable, Literal, Self

from kelir.generated.fusion_tables import RingEvent
from kelir.plugin_fusion import (
    FusedTrampoline,
    HookDescriptor,
    HookStage,
    LifecycleTag,
    PluginRegistration,
    TrapFilterSnapshot,
)
from kelir import VsockServiceEntry
from kelir.sdk_errors import EConfig

if TYPE_CHECKING:
    from kelir.model_plugin import BasePlugin

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

_SCOPE_KEYS = frozenset({"devices", "binaries", "paths"})


def _ring_for_value(value: str) -> RingEvent | None:
    for ring in RingEvent:
        if ring.value == value:
            return ring
    return None


# see: cans/02-plugin-model.md#Unified-builder
@dataclass
class _MountedPlugin:
    name: str
    host_only_verbs: list[str]
    vsock: list[Any]
    scope: Any
    requires: list[str]


# see: cans/02-plugin-model.md#Unified-builder
class TrapScope:
    def __init__(self, builder: KelirBuilder) -> None:
        self._builder = builder

    def on_trap(self, handler: Callable[..., Any]) -> TrapScope:
        self._builder._on("on_trap", handler)
        return self

    def on_process_exit(self, handler: Callable[..., Any]) -> TrapScope:
        self._builder._on("on_process_exit", handler)
        return self


# see: cans/02-plugin-model.md#Unified-builder
class FsScope:
    def __init__(self, builder: KelirBuilder) -> None:
        self._builder = builder

    def on_fs_lookup(self, handler: Callable[..., Any]) -> FsScope:
        self._builder._on("on_fs_lookup", handler)
        return self

    def on_fs_read(self, handler: Callable[..., Any]) -> FsScope:
        self._builder._on("on_fs_read", handler)
        return self

    def on_fs_write(self, handler: Callable[..., Any]) -> FsScope:
        self._builder._on("on_fs_write", handler)
        return self

    def on_fs_lock(self, handler: Callable[..., Any]) -> FsScope:
        self._builder._on("on_fs_lock", handler)
        return self


# see: cans/02-plugin-model.md#Unified-builder
class NetScope:
    def __init__(self, builder: KelirBuilder) -> None:
        self._builder = builder

    def on_net_outbound(self, handler: Callable[..., Any]) -> NetScope:
        self._builder._on("on_net_outbound", handler)
        return self

    def on_net_inbound(self, handler: Callable[..., Any]) -> NetScope:
        self._builder._on("on_net_inbound", handler)
        return self


# see: cans/02-plugin-model.md#Unified-builder
class ConsoleScope:
    def __init__(self, builder: KelirBuilder) -> None:
        self._builder = builder

    def on_console_input(self, handler: Callable[..., Any]) -> ConsoleScope:
        self._builder._on("on_console_input", handler)
        return self

    def on_console_output(self, handler: Callable[..., Any]) -> ConsoleScope:
        self._builder._on("on_console_output", handler)
        return self


# see: cans/02-plugin-model.md#Unified-builder
class ExecScope:
    def __init__(self, builder: KelirBuilder) -> None:
        self._builder = builder

    def on_exec_stdout(self, handler: Callable[..., Any]) -> ExecScope:
        self._builder._on("on_exec_stdout", handler)
        return self

    def on_exec_stderr(self, handler: Callable[..., Any]) -> ExecScope:
        self._builder._on("on_exec_stderr", handler)
        return self


# see: cans/02-plugin-model.md#Unified-builder
class HostScope:
    def __init__(self, builder: KelirBuilder) -> None:
        self._builder = builder

    def on_host_lsm_exec(self, handler: Callable[..., Any]) -> HostScope:
        self._builder._on("on_host_lsm_exec", handler)
        return self

    def on_host_pressure(self, handler: Callable[..., Any]) -> HostScope:
        self._builder._on("on_host_pressure", handler)
        return self

    def on_host_keyring_access(self, handler: Callable[..., Any]) -> HostScope:
        self._builder._on("on_host_keyring_access", handler)
        return self

    def on_host_preempt(self, handler: Callable[..., Any]) -> HostScope:
        self._builder._on("on_host_preempt", handler)
        return self


# see: cans/02-plugin-model.md#Unified-builder
class LifecycleScope:
    def __init__(self, builder: KelirBuilder) -> None:
        self._builder = builder

    def on_vm_init(self, handler: Callable[..., Any]) -> LifecycleScope:
        self._builder._on("on_vm_init", handler)
        return self

    def on_vm_ready(self, handler: Callable[..., Any]) -> LifecycleScope:
        self._builder._on("on_vm_ready", handler)
        return self

    def on_vm_shutdown(self, handler: Callable[..., Any]) -> LifecycleScope:
        self._builder._on("on_vm_shutdown", handler)
        return self

    def on_pre_commit(self, handler: Callable[..., Any]) -> LifecycleScope:
        self._builder._on("on_pre_commit", handler)
        return self

    def on_post_commit(self, handler: Callable[..., Any]) -> LifecycleScope:
        self._builder._on("on_post_commit", handler)
        return self


# see: cans/02-plugin-model.md#Unified-builder
class TelemetryScope:
    def __init__(self, builder: KelirBuilder) -> None:
        self._builder = builder

    def on_memory_pressure(self, handler: Callable[..., Any]) -> TelemetryScope:
        self._builder._on("on_memory_pressure", handler)
        return self

    def on_heartbeat(self, handler: Callable[..., Any]) -> TelemetryScope:
        self._builder._on("on_heartbeat", handler)
        return self

    def on_watchdog(self, handler: Callable[..., Any]) -> TelemetryScope:
        self._builder._on("on_watchdog", handler)
        return self


# see: cans/02-plugin-model.md#Unified-builder
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
        self._name: str | None = None
        self._requires: list[str] = []
        self._scope: Any = None
        self._context_fn: Callable[..., Any] | None = None
        self._fork_fn: Callable[[KelirBuilder], KelirBuilder] | None = None
        self._hooks: list[HookDescriptor] = []
        self._next_ordinal: int = 0
        self._vsock: list[tuple[Any, Callable[..., Any], int]] = []
        self._host_only_verbs: list[str] = []
        self._mounted: list[_MountedPlugin] = []
        self._plugins: list[object] = []
        self._plugin_registrations: list[PluginRegistration] = []
        self._session: Any = None
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

    @classmethod
    def plugin(cls, name: str) -> KelirBuilder:
        builder = cls()
        builder._name = name
        return builder

    def _record_host_only(self, verb: str) -> None:
        if self._name is not None and verb not in self._host_only_verbs:
            self._host_only_verbs.append(verb)

    def _on(self, verb: str, handler: Callable[..., Any]) -> KelirBuilder:
        ring_value, lifecycle_tag = _VERB_RINGS[verb]
        ring = _ring_for_value(ring_value)
        if ring is None:
            raise EConfig(f"ring '{ring_value}' for .{verb} has no Python RingEvent yet")
        self._hooks.append(
            HookDescriptor(
                ring=ring,
                stage=HookStage.SINGLE,
                ordinal=self._next_ordinal,
                plugin_name=self._name,
                lifecycle_tag=lifecycle_tag,
                run=handler,
            )
        )
        self._next_ordinal += 1
        return self

    def context(self, fn: Callable[..., Any]) -> KelirBuilder:
        self._context_fn = fn
        return self

    def require(self, plugin: type | str) -> KelirBuilder:
        required = plugin if isinstance(plugin, str) else str(getattr(plugin, "name", "") or plugin.__name__)
        if not required:
            raise EConfig(f"KelirBuilder '{self._name}' .require got an unnamed plugin")
        if required not in self._requires:
            self._requires.append(required)
        return self

    def scope(self, scope: Any) -> KelirBuilder:
        self._scope = scope
        return self

    def fork_with(self, fn: Callable[[KelirBuilder], KelirBuilder]) -> KelirBuilder:
        self._fork_fn = fn
        return self

    def on_trap(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_trap", handler)

    def on_process_exit(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_process_exit", handler)

    def on_fs_lookup(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_fs_lookup", handler)

    def on_fs_read(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_fs_read", handler)

    def on_fs_write(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_fs_write", handler)

    def on_fs_lock(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_fs_lock", handler)

    def on_net_outbound(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_net_outbound", handler)

    def on_net_inbound(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_net_inbound", handler)

    def on_console_input(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_console_input", handler)

    def on_console_output(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_console_output", handler)

    def on_exec_stdout(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_exec_stdout", handler)

    def on_exec_stderr(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_exec_stderr", handler)

    def on_host_lsm_exec(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_host_lsm_exec", handler)

    def on_host_pressure(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_host_pressure", handler)

    def on_host_keyring_access(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_host_keyring_access", handler)

    def on_vm_init(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_vm_init", handler)

    def on_vm_ready(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_vm_ready", handler)

    def on_vm_shutdown(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_vm_shutdown", handler)

    def on_pre_commit(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_pre_commit", handler)

    def on_post_commit(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_post_commit", handler)

    def on_host_preempt(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_host_preempt", handler)

    def on_memory_pressure(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_memory_pressure", handler)

    def on_heartbeat(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_heartbeat", handler)

    def on_watchdog(self, handler: Callable[..., Any]) -> KelirBuilder:
        return self._on("on_watchdog", handler)

    def vsock_service(self, spec: Any, handler: Callable[..., Any]) -> KelirBuilder:
        raw: Any = spec.get("port") if isinstance(spec, dict) else getattr(spec, "port", None)
        if raw is not None and (
            isinstance(raw, bool) or not isinstance(raw, int) or raw < 0 or raw > 0xFFFFFFFF
        ):
            raise EConfig(f"vsock service port {raw} outside u32")
        self._vsock.append((spec, handler, self._next_ordinal))
        self._next_ordinal += 1
        return self

    def trap(self, fn: Callable[[TrapScope], TrapScope]) -> KelirBuilder:
        fn(TrapScope(self))
        return self

    def fs(self, fn: Callable[[FsScope], FsScope]) -> KelirBuilder:
        fn(FsScope(self))
        return self

    def net(self, fn: Callable[[NetScope], NetScope]) -> KelirBuilder:
        fn(NetScope(self))
        return self

    def console(self, fn: Callable[[ConsoleScope], ConsoleScope]) -> KelirBuilder:
        fn(ConsoleScope(self))
        return self

    def exec(self, fn: Callable[[ExecScope], ExecScope]) -> KelirBuilder:
        fn(ExecScope(self))
        return self

    def host(self, fn: Callable[[HostScope], HostScope]) -> KelirBuilder:
        fn(HostScope(self))
        return self

    def lifecycle(self, fn: Callable[[LifecycleScope], LifecycleScope]) -> KelirBuilder:
        fn(LifecycleScope(self))
        return self

    def telemetry(self, fn: Callable[[TelemetryScope], TelemetryScope]) -> KelirBuilder:
        fn(TelemetryScope(self))
        return self

    def hook_descriptors(self) -> list[HookDescriptor]:
        descriptors = list(self._hooks)
        for spec, handler, ordinal in self._vsock:
            kind = _vsock_kind(spec)
            if kind is None or not 0 <= kind <= 0xFFFF:
                raise EConfig(f"vsock service for plugin '{self._name}' declares no u16 stream kind")
            descriptors.append(
                HookDescriptor(
                    ring=RingEvent.VSOCK_SERVICE,
                    stage=HookStage.SINGLE,
                    ordinal=ordinal,
                    plugin_name=self._name,
                    vsock_kind=kind,
                    run=handler,
                )
            )
        return sorted(descriptors, key=lambda hook: hook.ordinal)

    def fork_state(self) -> KelirBuilder:
        if self._fork_fn is not None:
            return self._fork_fn(self)
        clone = KelirBuilder.plugin(self._name) if self._name is not None else KelirBuilder()
        clone._requires = list(self._requires)
        clone._scope = self._scope
        clone._context_fn = self._context_fn
        clone._fork_fn = self._fork_fn
        clone._hooks = list(self._hooks)
        clone._next_ordinal = self._next_ordinal
        clone._vsock = list(self._vsock)
        clone._host_only_verbs = list(self._host_only_verbs)
        clone._mounted = [
            _MountedPlugin(
                name=mounted.name,
                host_only_verbs=list(mounted.host_only_verbs),
                vsock=list(mounted.vsock),
                scope=mounted.scope,
                requires=list(mounted.requires),
            )
            for mounted in self._mounted
        ]
        return clone

    def use(self, plugin: BasePlugin | KelirBuilder) -> Self:
        from dataclasses import replace

        if isinstance(plugin, KelirBuilder):
            name = plugin._name
            if name is None:
                raise EConfig("KelirBuilder.use() mounts a plugin-mode builder: use KelirBuilder.plugin(name)")
            if any(mounted.name == name for mounted in self._mounted):
                raise EConfig(f"plugin already mounted: {name}")
            for descriptor in plugin.hook_descriptors():
                self._hooks.append(replace(descriptor, ordinal=self._next_ordinal))
                self._next_ordinal += 1
            self._plugin_registrations.append(PluginRegistration(name=name, enabled=True))
            self._mounted.append(
                _MountedPlugin(
                    name=name,
                    host_only_verbs=list(plugin._host_only_verbs),
                    vsock=[spec for spec, _handler, _ordinal in plugin._vsock],
                    scope=plugin._scope,
                    requires=list(plugin._requires),
                )
            )
            self._mounted.extend(
                _MountedPlugin(
                    name=mounted.name,
                    host_only_verbs=list(mounted.host_only_verbs),
                    vsock=list(mounted.vsock),
                    scope=mounted.scope,
                    requires=list(mounted.requires),
                )
                for mounted in plugin._mounted
            )
            return self
        from kelir.model_plugin import hook_descriptors

        self._plugins.append(plugin)
        name = getattr(plugin, "name", None) or type(plugin).__name__
        enabled = bool(getattr(plugin, "enabled", True))
        self._plugin_registrations.append(
            PluginRegistration(name=str(name), enabled=enabled)
        )
        if enabled:
            for descriptor in hook_descriptors(plugin):
                self._hooks.append(replace(descriptor, ordinal=self._next_ordinal))
                self._next_ordinal += 1
        self._mounted.append(
            _MountedPlugin(name=str(name), host_only_verbs=[], vsock=[], scope=None, requires=[])
        )
        return self

    def plugin(self, plugin: BasePlugin | KelirBuilder) -> Self:
        return self.use(plugin)

    def detached(self, detached: bool) -> Self:
        self._record_host_only("detached")
        self._config["detached"] = detached
        return self

    def is_detached(self) -> bool:
        return bool(self._config.get("detached", False))

    def terminal(self, **kwargs: Any) -> Self:
        self._record_host_only("terminal")
        self._config["terminal"] = kwargs
        return self

    def storage(self, **kwargs: Any) -> Self:
        self._record_host_only("storage")
        self._config["storage"] = kwargs
        return self

    def wire(self, **kwargs: Any) -> Self:
        self._record_host_only("wire")
        self._config["wire"] = kwargs
        return self

    def params(self, model: type) -> Self:
        self._record_host_only("params")
        self._params_model = model
        return self

    def trap(
        self,
        fn_or_pattern: Callable[..., Any] | str | Any,
        pre: Callable[..., Awaitable[Any]] | None = None,
        post: Callable[..., Awaitable[Any]] | None = None,
    ) -> Self:
        if callable(fn_or_pattern) and pre is None and post is None:
            fn_or_pattern(TrapScope(self))
            return self
        self._record_host_only("trap")
        pattern = fn_or_pattern
        snapshot = (
            pattern.snapshot()
            if hasattr(pattern, "snapshot")
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
        self._record_host_only("register_custom_action")
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

    def register_stream_kind(
        self,
        kind: int,
        schema: dict[str, Any],
        handler: Callable[[bytes], Awaitable[bytes]],
    ) -> Self:
        self._record_host_only("register_stream_kind")
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

    _LIFECYCLE_TAGS: dict[str, Literal["trap-deny"]] = {
        "on_trap_deny": "trap-deny",
    }

    def _lifecycle_decorator(
        self, name: str
    ) -> Callable[[Callable[..., Awaitable[Any]]], Callable[..., Awaitable[Any]]]:
        tag = self._LIFECYCLE_TAGS.get(name)
        if tag is None:
            raise EConfig(f"unknown lifecycle hook {name}")

        def decorator(
            fn: Callable[..., Awaitable[Any]],
        ) -> Callable[..., Awaitable[Any]]:
            self._record_host_only(name)
            self._hooks.append(
                HookDescriptor(
                    ring=RingEvent.LIFECYCLE,
                    stage=HookStage.SINGLE,
                    ordinal=self._next_ordinal,
                    lifecycle_tag=tag,
                    run=fn,
                )
            )
            self._next_ordinal += 1
            return fn

        return decorator

    @property
    def on_trap_deny(self) -> Callable[[Callable[..., Awaitable[Any]]], Callable[..., Awaitable[Any]]]:
        return self._lifecycle_decorator("on_trap_deny")

    @property
    def vsock_services(self) -> list[VsockServiceEntry]:
        return [spec for spec, _, _ in self._vsock if isinstance(spec, VsockServiceEntry)]

    def fuse(self) -> FusedTrampoline:
        return FusedTrampoline.fuse(self.hook_descriptors())

    async def boot(self) -> Any:
        if self._name is not None:
            raise EConfig("boot requires a host-mode builder")
        self._validate()
        registered = {registration.name for registration in self._plugin_registrations}
        for required in self._requires:
            if required not in registered:
                raise EConfig(f"missing prerequisite: {required} required by host")
        for plugin in self._plugins:
            name = getattr(plugin, "name", None) or type(plugin).__name__
            required_names = getattr(plugin, "requires", ())
            if isinstance(required_names, str):
                required_names = (required_names,)
            for required in required_names:
                if required not in registered:
                    raise EConfig(f"missing prerequisite: {required} required by {name}")
        for mounted in self._mounted:
            for required in mounted.requires:
                if required not in registered:
                    raise EConfig(f"missing prerequisite: {required} required by {mounted.name}")
        descriptor = self.fuse().describe()
        # session boot follows the live KelirBuilder.boot() tail (native
        # bridge); the descriptor above is its validated input.
        return descriptor

    def _validate(self) -> None:
        for spec, _handler, _ordinal in self._vsock:
            _validate_vsock_spec(self._name, spec)
        if self._scope is not None:
            _validate_scope(self._name, self._scope)
        for mounted in self._mounted:
            for spec in mounted.vsock:
                _validate_vsock_spec(mounted.name, spec)
            if mounted.scope is not None:
                _validate_scope(mounted.name, mounted.scope)
            if mounted.host_only_verbs:
                verb = mounted.host_only_verbs[0]
                raise EConfig(f"{mounted.name}: host-only verb '{verb}' used on a plugin-mode builder")


def _vsock_kind(spec: Any) -> int | None:
    if isinstance(spec, dict):
        raw: Any = spec.get("kind", spec.get("port"))
    else:
        raw = getattr(spec, "kind", getattr(spec, "port", None))
    if isinstance(raw, bool) or not isinstance(raw, int):
        return None
    return raw


def _validate_vsock_spec(name: str | None, spec: Any) -> None:
    if isinstance(spec, dict):
        port = spec.get("port")
        service = spec.get("service", "")
    else:
        port = getattr(spec, "port", None)
        service = getattr(spec, "service", "")
    if isinstance(port, bool) or not isinstance(port, int) or not 1 <= port <= 65535:
        raise EConfig(f"vsock service for plugin '{name}' declares a port outside 1..=65535")
    if not isinstance(service, str) or not service:
        raise EConfig(f"vsock service for plugin '{name}' declares an empty service name")


def _validate_scope(name: str | None, scope: Any) -> None:
    if not isinstance(scope, dict):
        raise EConfig(f"mount scope for plugin '{name}' is not a mapping")
    unknown = set(scope) - _SCOPE_KEYS
    if unknown:
        raise EConfig(f"mount scope for plugin '{name}' declares unknown keys: {sorted(unknown)}")
    for key, paths in scope.items():
        if not isinstance(paths, list) or not all(isinstance(entry, str) for entry in paths):
            raise EConfig(f"mount scope for plugin '{name}' key '{key}' is not a list of strings")


# see: cans/02-plugin-model.md#Unified-builder
# Deleted shapes: PluginBuilder, _PluginWrapper, plugin_builder.py, the
# _built flag and _check_open machinery, build(). Nothing is documented
# as an alternative: the old shapes are gone, not deprecated.
# Mount: use() merges a plugin-mode builder by value. fuse() sorts each
# ring by ordinal and throws on a duplicate ordinal, so use()
# re-ordinals the mounted plugin's hooks into the host's sequence as it
# merges them (dataclasses.replace keeps the plugin builder's own
# ordinals untouched). Mounting the same plugin twice raises EConfig
# `plugin already mounted: <name>`. The missing-prerequisite check is at
# boot() with `missing prerequisite: <dep> required by <plugin>`; that
# text is already the live boot() text in packages/py/kelir/__init__.py
# (the DX-round note claiming the old text was stale). boot() is the one
# validation: the vsock triple (port 1..=65535, non-empty service)
# applies to host-registered and plugin services alike
# (vsock_service keeps the live u32 call-site check as the
# type-range guard), the scope triple (non-empty paths), and host-only
# verbs recorded on a plugin-mode builder, rejected with `<plugin>:
# host-only verb '<verb>' used on a plugin-mode builder`. An empty
# builder (no handlers) still boots. The vsock spec is (port, service,
# schema) in Python: the schema field is plugin-level in TS/Python and
# absent in Rust. Trait-object plugins mounted via use() validate
# vsock/scope at bind time through the binder, not at boot(): their specs
# are only visible inside hook_descriptors().
# trap() dispatches on its arguments: one callable and no pre/post is the
# trap scope entry point; (pattern, pre, post) is the host-only trap
# registration (pattern filter, PRE/POST stages). The live host's
# on_vm_init / on_vm_ready / on_vm_shutdown / on_pre_commit /
# on_post_commit decorator properties are superseded by the ring-verb
# methods of the same names (plain method form, returning Self);
# on_trap_deny stays the host-only decorator property: it is not one of
# the 24 rings. Registration-time arity check: each on_* verifies the
# handler arity via inspect.signature and raises EConfig at registration
# on mismatch. Vsock ordinals are assigned at registration so
# hook_descriptors() is pure; the ring-missing EConfig now raises at
# registration (it raised at _descriptors() under build()).
