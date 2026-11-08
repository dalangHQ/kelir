# see: cans/02-plugin-model.md#Foreign-plugin-proxy
# see: cans/01-architecture.md#Foreign-runtime-embedding
# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from typing import Any, Callable, Literal

from kelir.errors import EConfig


class RingEvent(str, Enum):
    TRAP = "trap"
    TRAP_DENY = "trap-deny"
    PROCESS_EXIT = "process-exit"
    FS_LOOKUP = "fs-lookup"
    FS_READ = "fs-read"
    FS_WRITE = "fs-write"
    NET_OUTBOUND = "net-outbound"
    NET_INBOUND = "net-inbound"
    CONSOLE_INPUT = "console-input"
    CONSOLE_OUTPUT = "console-output"
    EXEC_STDOUT = "exec-stdout"
    EXEC_STDERR = "exec-stderr"
    HEARTBEAT = "heartbeat"
    LIFECYCLE = "lifecycle"
    WATCHDOG = "watchdog"
    CUSTOM_ACTION = "custom-action"
    VSOCK_SERVICE = "vsock-service"


class HookStage(str, Enum):
    SINGLE = "single"
    PRE = "pre"
    POST = "post"


LifecycleTag = Literal[
    "vm-init",
    "vm-ready",
    "vm-shutdown",
    "trap-deny",
    "pre-commit",
    "post-commit",
]


@dataclass(frozen=True)
class TrapFilterSnapshot:
    binary: str | None = None
    args: tuple[str, ...] = ()
    pattern: str | None = None


@dataclass
class HookDescriptor:
    ring: RingEvent
    stage: HookStage
    ordinal: int
    plugin_name: str | None = None
    filter: TrapFilterSnapshot | None = None
    lifecycle_tag: LifecycleTag | None = None
    custom_action_name: str | None = None
    vsock_kind: int | None = None
    run: Callable[[Any], Any] = lambda _event: None


@dataclass(frozen=True)
class PluginRegistration:
    name: str
    enabled: bool = True


@dataclass(frozen=True)
class FusedRingChain:
    ring: RingEvent
    hooks: tuple[HookDescriptor, ...]


def _validate_hook(hook: HookDescriptor) -> None:
    if hook.ring is RingEvent.CUSTOM_ACTION and not hook.custom_action_name:
        raise EConfig(
            f"custom action hook on ordinal {hook.ordinal} declares no action name"
        )
    if hook.ring is RingEvent.VSOCK_SERVICE:
        if hook.vsock_kind is None:
            raise EConfig(
                f"vsock service hook on ordinal {hook.ordinal} declares no stream kind"
            )
        if not 0 <= hook.vsock_kind <= 0xFFFF:
            raise EConfig(
                f"vsock service hook on ordinal {hook.ordinal} declares kind "
                f"{hook.vsock_kind} outside u16"
            )
    if hook.ring is RingEvent.LIFECYCLE and not hook.lifecycle_tag:
        raise EConfig(
            f"lifecycle hook on ordinal {hook.ordinal} declares no lifecycle tag"
        )


# see: cans/02-plugin-model.md#Foreign-plugin-proxy
@dataclass(frozen=True)
class FusedTrampoline:
    chains: tuple[FusedRingChain, ...]

    @classmethod
    def fuse(cls, hooks: list[HookDescriptor]) -> FusedTrampoline:
        by_ring: dict[RingEvent, list[HookDescriptor]] = {}
        for hook in hooks:
            _validate_hook(hook)
            by_ring.setdefault(hook.ring, []).append(hook)
        chains: list[FusedRingChain] = []
        for ring, ring_hooks in by_ring.items():
            ordered = sorted(ring_hooks, key=lambda hook: hook.ordinal)
            for earlier, later in zip(ordered, ordered[1:]):
                if earlier.ordinal == later.ordinal:
                    raise EConfig(
                        f"fused trampoline ring {ring.value} has duplicate ordinal "
                        f"{later.ordinal}"
                    )
            chains.append(FusedRingChain(ring=ring, hooks=tuple(ordered)))
        return cls(chains=tuple(chains))

    def crossings_per_ring_event(self, ring: RingEvent) -> int:
        return 1 if any(chain.ring is ring for chain in self.chains) else 0

    def hook_count(self, ring: RingEvent) -> int:
        for chain in self.chains:
            if chain.ring is ring:
                return len(chain.hooks)
        return 0

    # TODO (L3): compile each fused chain into the single flat closure that runs it per ring event.
    def describe(self) -> dict[str, Any]:
        return {
            "rings": [
                {
                    "ring": chain.ring.value,
                    "hook_count": len(chain.hooks),
                    "hooks": [
                        {
                            "ordinal": hook.ordinal,
                            "stage": hook.stage.value,
                            "plugin_name": hook.plugin_name,
                            "filter": (
                                None
                                if hook.filter is None
                                else {
                                    "binary": hook.filter.binary,
                                    "args": list(hook.filter.args),
                                    "pattern": hook.filter.pattern,
                                }
                            ),
                            "lifecycle_tag": hook.lifecycle_tag,
                            "custom_action_name": hook.custom_action_name,
                            "vsock_kind": hook.vsock_kind,
                        }
                        for hook in chain.hooks
                    ],
                }
                for chain in self.chains
            ]
        }
