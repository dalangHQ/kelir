# ILLUSTRATIVE, NON-STANDALONE — do not import this file directly.
# It pins the Python fluent plugin-creation builder surface; the live shape
# is packages/py/kelir/plugin_builder.py (Agent 5) and the normative
# contract is cans/02-plugin-model.md#Fluent-plugin-creation-builder.
#
# Target shape: the 30-verb closed set (6 structural + 24 ring), the 8
# closure-scoped entry points, and the validating build. The scope entry
# points, the ordered-list handler store (append, never last-wins), and
# multi-vsock are the A5 contract (DX-5): the live builder still stores one
# handler per verb in a dict and a single vsock slot. Flat verbs stay; a
# nested chain and the equivalent flat chain produce identical descriptors
# (same rings, same ordinals). The canonical chaos plugin is not built
# here: it lives in cans/artifacts/examples/fluent-builder-chaos.py, the
# one shape shared by all three SDKs.
#
# Owned by: cans/02-plugin-model.md#Fluent-plugin-creation-builder
# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
# see: cans/03-sdk-architecture.md#Fluent-plugin-surface
# see: cans/artifacts/examples/fluent-builder-chaos.py#build_chaos_plugin

from __future__ import annotations

from typing import Any, Callable

from kelir.generated.fusion_tables import RingEvent
from kelir.plugin_fusion import HookDescriptor, HookStage, LifecycleTag
from kelir.sdk_errors import EConfig

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


# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
class _PluginWrapper:
    def __init__(
        self,
        name: str,
        requires: tuple[str, ...],
        scope: Any,
        context_fn: Callable[..., Any] | None,
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
            return self._fork_fn(self)
        return _PluginWrapper(
            self.name, self.requires, self.scope, self._context_fn, self._fork_fn, list(self._descriptors)
        )


# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
class TrapScope:
    def __init__(self, builder: PluginBuilder) -> None:
        self._builder = builder

    def on_trap(self, handler: Callable[..., Any]) -> TrapScope:
        self._builder._on("on_trap", handler)
        return self

    def on_process_exit(self, handler: Callable[..., Any]) -> TrapScope:
        self._builder._on("on_process_exit", handler)
        return self


# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
class FsScope:
    def __init__(self, builder: PluginBuilder) -> None:
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


# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
class NetScope:
    def __init__(self, builder: PluginBuilder) -> None:
        self._builder = builder

    def on_net_outbound(self, handler: Callable[..., Any]) -> NetScope:
        self._builder._on("on_net_outbound", handler)
        return self

    def on_net_inbound(self, handler: Callable[..., Any]) -> NetScope:
        self._builder._on("on_net_inbound", handler)
        return self


# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
class ConsoleScope:
    def __init__(self, builder: PluginBuilder) -> None:
        self._builder = builder

    def on_console_input(self, handler: Callable[..., Any]) -> ConsoleScope:
        self._builder._on("on_console_input", handler)
        return self

    def on_console_output(self, handler: Callable[..., Any]) -> ConsoleScope:
        self._builder._on("on_console_output", handler)
        return self


# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
class ExecScope:
    def __init__(self, builder: PluginBuilder) -> None:
        self._builder = builder

    def on_exec_stdout(self, handler: Callable[..., Any]) -> ExecScope:
        self._builder._on("on_exec_stdout", handler)
        return self

    def on_exec_stderr(self, handler: Callable[..., Any]) -> ExecScope:
        self._builder._on("on_exec_stderr", handler)
        return self


# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
class HostScope:
    def __init__(self, builder: PluginBuilder) -> None:
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


# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
class LifecycleScope:
    def __init__(self, builder: PluginBuilder) -> None:
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


# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
class TelemetryScope:
    def __init__(self, builder: PluginBuilder) -> None:
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


# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
class PluginBuilder:
    def __init__(self, name: str) -> None:
        self._name = name
        self._requires: list[str] = []
        self._scope: Any = None
        self._context_fn: Callable[..., Any] | None = None
        self._fork_fn: Callable[[_PluginWrapper], _PluginWrapper] | None = None
        self._handlers: list[tuple[str, int, Callable[..., Any]]] = []
        self._next_ordinal: int = 0
        self._vsock: list[tuple[Any, Callable[..., Any]]] = []
        self._built: bool = False

    def _check_open(self, verb: str) -> None:
        if self._built:
            raise EConfig(f"PluginBuilder '{self._name}' is already built: .{verb} after .build()")

    def _on(self, verb: str, handler: Callable[..., Any]) -> PluginBuilder:
        self._check_open(verb)
        self._handlers.append((verb, self._next_ordinal, handler))
        self._next_ordinal += 1
        return self

    def context(self, fn: Callable[..., Any]) -> PluginBuilder:
        self._check_open("context")
        self._context_fn = fn
        return self

    def require(self, plugin: type | str) -> PluginBuilder:
        self._check_open("require")
        required = plugin if isinstance(plugin, str) else str(getattr(plugin, "name", "") or plugin.__name__)
        if not required:
            raise EConfig(f"PluginBuilder '{self._name}' .require got an unnamed plugin")
        if required not in self._requires:
            self._requires.append(required)
        return self

    def scope(self, scope: Any) -> PluginBuilder:
        self._check_open("scope")
        self._scope = scope
        return self

    def fork_with(self, fn: Callable[[_PluginWrapper], _PluginWrapper]) -> PluginBuilder:
        self._check_open("fork_with")
        self._fork_fn = fn
        return self

    def on_trap(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_trap", handler)

    def on_process_exit(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_process_exit", handler)

    def on_fs_lookup(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_fs_lookup", handler)

    def on_fs_read(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_fs_read", handler)

    def on_fs_write(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_fs_write", handler)

    def on_fs_lock(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_fs_lock", handler)

    def on_net_outbound(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_net_outbound", handler)

    def on_net_inbound(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_net_inbound", handler)

    def on_console_input(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_console_input", handler)

    def on_console_output(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_console_output", handler)

    def on_exec_stdout(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_exec_stdout", handler)

    def on_exec_stderr(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_exec_stderr", handler)

    def on_host_lsm_exec(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_host_lsm_exec", handler)

    def on_host_pressure(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_host_pressure", handler)

    def on_host_keyring_access(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_host_keyring_access", handler)

    def on_vm_init(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_vm_init", handler)

    def on_vm_ready(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_vm_ready", handler)

    def on_vm_shutdown(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_vm_shutdown", handler)

    def on_pre_commit(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_pre_commit", handler)

    def on_post_commit(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_post_commit", handler)

    def on_host_preempt(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_host_preempt", handler)

    def on_memory_pressure(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_memory_pressure", handler)

    def on_heartbeat(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_heartbeat", handler)

    def on_watchdog(self, handler: Callable[..., Any]) -> PluginBuilder:
        return self._on("on_watchdog", handler)

    def vsock_service(self, spec: Any, handler: Callable[..., Any]) -> PluginBuilder:
        self._check_open("vsock_service")
        self._vsock.append((spec, handler))
        return self

    def trap(self, fn: Callable[[TrapScope], TrapScope]) -> PluginBuilder:
        self._check_open("trap")
        fn(TrapScope(self))
        return self

    def fs(self, fn: Callable[[FsScope], FsScope]) -> PluginBuilder:
        self._check_open("fs")
        fn(FsScope(self))
        return self

    def net(self, fn: Callable[[NetScope], NetScope]) -> PluginBuilder:
        self._check_open("net")
        fn(NetScope(self))
        return self

    def console(self, fn: Callable[[ConsoleScope], ConsoleScope]) -> PluginBuilder:
        self._check_open("console")
        fn(ConsoleScope(self))
        return self

    def exec(self, fn: Callable[[ExecScope], ExecScope]) -> PluginBuilder:
        self._check_open("exec")
        fn(ExecScope(self))
        return self

    def host(self, fn: Callable[[HostScope], HostScope]) -> PluginBuilder:
        self._check_open("host")
        fn(HostScope(self))
        return self

    def lifecycle(self, fn: Callable[[LifecycleScope], LifecycleScope]) -> PluginBuilder:
        self._check_open("lifecycle")
        fn(LifecycleScope(self))
        return self

    def telemetry(self, fn: Callable[[TelemetryScope], TelemetryScope]) -> PluginBuilder:
        self._check_open("telemetry")
        fn(TelemetryScope(self))
        return self

    def _descriptors(self) -> list[HookDescriptor]:
        descriptors: list[HookDescriptor] = []
        for verb, ordinal, handler in sorted(self._handlers, key=lambda entry: entry[1]):
            ring_value, lifecycle_tag = _VERB_RINGS[verb]
            ring = _ring_for_value(ring_value)
            if ring is None:
                raise EConfig(f"ring '{ring_value}' for .{verb} has no Python RingEvent yet")
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
        for spec, handler in self._vsock:
            kind = _vsock_kind(spec)
            if kind is None or not 0 <= kind <= 0xFFFF:
                raise EConfig(f"vsock service for plugin '{self._name}' declares no u16 stream kind")
            descriptors.append(
                HookDescriptor(
                    ring=RingEvent.VSOCK_SERVICE,
                    stage=HookStage.SINGLE,
                    ordinal=self._next_ordinal,
                    plugin_name=self._name,
                    vsock_kind=kind,
                    run=handler,
                )
            )
            self._next_ordinal += 1
        return sorted(descriptors, key=lambda hook: hook.ordinal)

    def build(self) -> _PluginWrapper:
        self._check_open("build")
        if self._scope is not None:
            _validate_scope(self._name, self._scope)
        for spec, _handler in self._vsock:
            _validate_vsock_spec(self._name, spec)
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


def _vsock_kind(spec: Any) -> int | None:
    if isinstance(spec, dict):
        raw: Any = spec.get("kind", spec.get("port"))
    else:
        raw = getattr(spec, "kind", getattr(spec, "port", None))
    if isinstance(raw, bool) or not isinstance(raw, int):
        return None
    return raw


def _validate_vsock_spec(name: str, spec: Any) -> None:
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


def _validate_scope(name: str, scope: Any) -> None:
    if not isinstance(scope, dict):
        raise EConfig(f"mount scope for plugin '{name}' is not a mapping")
    unknown = set(scope) - _SCOPE_KEYS
    if unknown:
        raise EConfig(f"mount scope for plugin '{name}' declares unknown keys: {sorted(unknown)}")
    for key, paths in scope.items():
        if not isinstance(paths, list) or not all(isinstance(entry, str) for entry in paths):
            raise EConfig(f"mount scope for plugin '{name}' key '{key}' is not a list of strings")


# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder
# EConfig triggers pinned by the contract:
#   - .require(P) naming a plugin absent from the sandbox registrations
#     raises EConfig at KelirBuilder.boot() with the message
#     `missing prerequisite: <dep> required by <plugin>`; the live text in
#     packages/py/kelir/__init__.py boot() still reads `missing
#     prerequisite '<dep>' required by plugin '<plugin>'` until A5 lands it.
#   - Any verb call after .build() raises EConfig (post-build mutation).
#   - .build() before .use(): KelirBuilder.use() accepts the _PluginWrapper
#     (post-build) only; passing a pre-build PluginBuilder raises EConfig.
#   - Registration-time arity check (A5 contract DX-5): each on_* verifies
#     the handler arity via inspect.signature and raises EConfig at
#     registration on mismatch.
#   - The generated PLUGIN_BUILDER_VERBS table is deleted: the live table
#     is _VERB_RINGS above (24 verbs, with lifecycle tags); the generated
#     duplicate (15 verbs, no tags) was never consulted by plugin_builder.py.
