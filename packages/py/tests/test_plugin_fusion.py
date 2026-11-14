# see: cans/02-plugin-model.md#Foreign-plugin-proxy
# see: cans/02-plugin-model.md#Segregated-ring-traits
from __future__ import annotations

import dataclasses
from typing import Any, cast

import pytest

from kelir import KelirBuilder
from kelir.sdk_errors import EConfig
from kelir.plugin_fusion import (
    FusedTrampoline,
    HookDescriptor,
    HookStage,
    LifecycleTag,
    PluginRegistration,
    RingEvent,
    TrapFilterSnapshot,
)
from kelir.generated.fusion_tables import LIFECYCLE_TAGS
from kelir.plugin_model import BasePlugin, WatchdogEvent, WatchdogVerdict, hook_descriptors


async def _noop(event: Any) -> None:
    return None


def test_ring_event_wire_values() -> None:
    assert RingEvent.TRAP.value == "trap"
    assert RingEvent.TRAP_DENY.value == "trap-deny"
    assert RingEvent.LIFECYCLE.value == "lifecycle"
    assert RingEvent.WATCHDOG.value == "watchdog"
    assert RingEvent.CUSTOM_ACTION.value == "custom-action"
    assert RingEvent.VSOCK_SERVICE.value == "vsock-service"
    assert RingEvent.FS_LOCK.value == "fs-lock"
    assert RingEvent.HOST_LSM_EXEC.value == "host-lsm-exec"
    assert RingEvent.HOST_PRESSURE.value == "host-pressure"
    assert RingEvent.HOST_KEYRING_ACCESS.value == "host-keyring-access"
    assert RingEvent.HOST_PREEMPT.value == "host-preempt"
    assert RingEvent.MEMORY_PRESSURE.value == "memory-pressure"
    assert len(RingEvent) == 23


def test_hook_stage_values() -> None:
    assert HookStage.PRE.value == "pre"
    assert HookStage.POST.value == "post"
    assert HookStage.SINGLE.value == "single"


def test_lifecycle_tags_are_the_six_l2_tags() -> None:
    assert LIFECYCLE_TAGS == (
        "vm-init",
        "vm-ready",
        "vm-shutdown",
        "trap-deny",
        "pre-commit",
        "post-commit",
    )


def test_trap_filter_snapshot_defaults_and_immutability() -> None:
    snapshot = TrapFilterSnapshot()
    assert snapshot.binary is None
    assert snapshot.args == ()
    assert snapshot.pattern is None
    mutable_view = cast(Any, snapshot)
    with pytest.raises(dataclasses.FrozenInstanceError):
        mutable_view.binary = "git"


def test_hook_descriptor_defaults() -> None:
    hook = HookDescriptor(ring=RingEvent.TRAP, stage=HookStage.PRE, ordinal=3)
    assert hook.plugin_name is None
    assert hook.filter is None
    assert hook.lifecycle_tag is None
    assert hook.custom_action_name is None
    assert hook.vsock_kind is None
    assert hook.run(None) is None


def test_plugin_registration_defaults_and_immutability() -> None:
    registration = PluginRegistration(name="policy-matrix")
    assert registration.name == "policy-matrix"
    assert registration.enabled is True
    assert PluginRegistration(name="x", enabled=False).enabled is False
    mutable_view = cast(Any, registration)
    with pytest.raises(dataclasses.FrozenInstanceError):
        mutable_view.enabled = False


def test_fuse_of_empty_hook_list() -> None:
    trampoline = FusedTrampoline.fuse([])
    assert trampoline.chains == ()
    assert trampoline.describe() == {"rings": []}
    assert trampoline.crossings_per_ring_event(RingEvent.TRAP) == 0


def test_runs_threads_each_hook_callable_by_ring_and_ordinal() -> None:
    seen: list[Any] = []
    hooks = [
        HookDescriptor(
            ring=RingEvent.TRAP,
            stage=HookStage.PRE,
            ordinal=1,
            run=lambda event: seen.append(event),
        ),
        HookDescriptor(
            ring=RingEvent.TRAP,
            stage=HookStage.PRE,
            ordinal=0,
            run=_noop,
        ),
    ]
    runs = FusedTrampoline.fuse(hooks).runs()
    assert [(entry["ring"], entry["ordinal"]) for entry in runs] == [
        ("trap", 0),
        ("trap", 1),
    ]
    runs[1]["run"]("trap", 1)
    assert seen == [{"ring": "trap", "ordinal": 1}]


def test_fuse_orders_hooks_by_ordinal_within_ring() -> None:
    hooks = [
        HookDescriptor(ring=RingEvent.TRAP, stage=HookStage.PRE, ordinal=5, run=_noop),
        HookDescriptor(ring=RingEvent.TRAP, stage=HookStage.PRE, ordinal=1, run=_noop),
        HookDescriptor(ring=RingEvent.TRAP, stage=HookStage.POST, ordinal=3, run=_noop),
    ]
    trampoline = FusedTrampoline.fuse(hooks)
    assert len(trampoline.chains) == 1
    chain = trampoline.chains[0]
    assert chain.ring is RingEvent.TRAP
    assert [hook.ordinal for hook in chain.hooks] == [1, 3, 5]
    assert trampoline.hook_count(RingEvent.TRAP) == 3


def test_fuse_rejects_duplicate_ordinals_in_one_ring() -> None:
    hooks = [
        HookDescriptor(ring=RingEvent.TRAP, stage=HookStage.PRE, ordinal=0, run=_noop),
        HookDescriptor(ring=RingEvent.TRAP, stage=HookStage.POST, ordinal=0, run=_noop),
    ]
    with pytest.raises(EConfig, match=r"duplicate ordinal 0"):
        FusedTrampoline.fuse(hooks)


def test_crossings_per_ring_event_counts_ring_presence() -> None:
    hooks = [HookDescriptor(ring=RingEvent.TRAP, stage=HookStage.PRE, ordinal=0, run=_noop)]
    trampoline = FusedTrampoline.fuse(hooks)
    assert trampoline.crossings_per_ring_event(RingEvent.TRAP) == 1
    assert trampoline.crossings_per_ring_event(RingEvent.WATCHDOG) == 0
    assert trampoline.hook_count(RingEvent.WATCHDOG) == 0


def test_fuse_rejects_custom_action_without_name() -> None:
    hooks = [
        HookDescriptor(
            ring=RingEvent.CUSTOM_ACTION, stage=HookStage.SINGLE, ordinal=0, run=_noop
        )
    ]
    with pytest.raises(EConfig, match=r"declares no action name"):
        FusedTrampoline.fuse(hooks)


def test_fuse_rejects_vsock_service_without_kind() -> None:
    hooks = [
        HookDescriptor(
            ring=RingEvent.VSOCK_SERVICE, stage=HookStage.SINGLE, ordinal=0, run=_noop
        )
    ]
    with pytest.raises(EConfig, match=r"declares no stream kind"):
        FusedTrampoline.fuse(hooks)


def test_fuse_rejects_vsock_service_kind_outside_u16() -> None:
    hooks = [
        HookDescriptor(
            ring=RingEvent.VSOCK_SERVICE,
            stage=HookStage.SINGLE,
            ordinal=0,
            vsock_kind=0x10000,
            run=_noop,
        )
    ]
    with pytest.raises(EConfig, match=r"outside u16"):
        FusedTrampoline.fuse(hooks)


def test_fuse_rejects_unknown_lifecycle_tag() -> None:
    hooks = [
        HookDescriptor(
            ring=RingEvent.LIFECYCLE,
            stage=HookStage.SINGLE,
            ordinal=0,
            lifecycle_tag=cast(LifecycleTag, "vm-bogus"),
            run=_noop,
        )
    ]
    with pytest.raises(EConfig, match=r"unknown lifecycle tag 'vm-bogus'"):
        FusedTrampoline.fuse(hooks)


def test_fuse_accepts_every_declared_lifecycle_tag() -> None:
    hooks = [
        HookDescriptor(
            ring=RingEvent.LIFECYCLE,
            stage=HookStage.SINGLE,
            ordinal=ordinal,
            lifecycle_tag=cast(LifecycleTag, tag),
            run=_noop,
        )
        for ordinal, tag in enumerate(LIFECYCLE_TAGS)
    ]
    trampoline = FusedTrampoline.fuse(hooks)
    assert trampoline.hook_count(RingEvent.LIFECYCLE) == 6
    assert [hook.lifecycle_tag for hook in trampoline.chains[0].hooks] == list(
        LIFECYCLE_TAGS
    )


def test_describe_emits_wire_shape() -> None:
    hooks = [
        HookDescriptor(
            ring=RingEvent.TRAP,
            stage=HookStage.PRE,
            ordinal=0,
            plugin_name="policy-matrix",
            filter=TrapFilterSnapshot(binary="git", args=("-c",)),
            run=_noop,
        ),
        HookDescriptor(
            ring=RingEvent.CUSTOM_ACTION,
            stage=HookStage.SINGLE,
            ordinal=1,
            custom_action_name="quarantine",
            run=_noop,
        ),
    ]
    trampoline = FusedTrampoline.fuse(hooks)
    assert trampoline.describe() == {
        "rings": [
            {
                "ring": "trap",
                "hook_count": 1,
                "hooks": [
                    {
                        "ordinal": 0,
                        "stage": "pre",
                        "plugin_name": "policy-matrix",
                        "filter": {"binary": "git", "args": ["-c"], "pattern": None},
                        "lifecycle_tag": None,
                        "custom_action_name": None,
                        "vsock_kind": None,
                    }
                ],
            },
            {
                "ring": "custom-action",
                "hook_count": 1,
                "hooks": [
                    {
                        "ordinal": 1,
                        "stage": "single",
                        "plugin_name": None,
                        "filter": None,
                        "lifecycle_tag": None,
                        "custom_action_name": "quarantine",
                        "vsock_kind": None,
                    }
                ],
            },
        ]
    }


def test_hook_descriptors_empty_for_vanilla_plugin() -> None:
    assert hook_descriptors(BasePlugin()) == []
    assert hook_descriptors(object()) == []


class WatchdogOnlyPlugin(BasePlugin[None, None]):
    name = "watchdog-only"

    async def on_watchdog(self, event: WatchdogEvent) -> WatchdogVerdict:
        return WatchdogVerdict.Mitigate


class LifecyclePlugin(BasePlugin[None, None]):
    name = "lifecycle-probe"

    async def on_vm_init(self, ctx: Any) -> None:
        return None

    async def on_vm_ready(self, ctx: Any) -> None:
        return None


def test_hook_descriptors_maps_watchdog_override() -> None:
    descriptors = hook_descriptors(WatchdogOnlyPlugin())
    assert len(descriptors) == 1
    assert descriptors[0].ring is RingEvent.WATCHDOG
    assert descriptors[0].stage is HookStage.SINGLE
    assert descriptors[0].ordinal == 0
    assert descriptors[0].plugin_name == "watchdog-only"
    assert descriptors[0].lifecycle_tag is None


def test_hook_descriptors_maps_lifecycle_overrides_with_tags() -> None:
    descriptors = hook_descriptors(LifecyclePlugin())
    assert len(descriptors) == 2
    assert [d.ring for d in descriptors] == [RingEvent.LIFECYCLE, RingEvent.LIFECYCLE]
    assert [d.lifecycle_tag for d in descriptors] == ["vm-init", "vm-ready"]
    assert [d.ordinal for d in descriptors] == [0, 1]
    assert descriptors[0].plugin_name == "lifecycle-probe"


def test_builder_use_collects_plugin_registrations_and_hooks() -> None:
    builder = KelirBuilder().use(WatchdogOnlyPlugin()).use(LifecyclePlugin())
    trampoline = builder.fuse()
    assert trampoline.hook_count(RingEvent.WATCHDOG) == 1
    assert trampoline.hook_count(RingEvent.LIFECYCLE) == 2
    registrations = builder._plugin_registrations
    assert [r.name for r in registrations] == ["watchdog-only", "lifecycle-probe"]
    assert all(r.enabled for r in registrations)
