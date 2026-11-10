# see: cans/02-plugin-model.md#KelirPlugin-trait
# see: cans/03-sdk-architecture.md#Prerequisite-gating-across-languages
from __future__ import annotations

import asyncio
from pathlib import Path
from typing import Any

import pytest

from kelir import FilterAction, HandlerContext, KelirBuilder, PluginBuilder, Verdict
from kelir.model_plugin import (
    BasePlugin,
    WatchdogEvent,
    WatchdogVerdict,
    hook_descriptors,
)
from kelir.plugin_builder import _PluginWrapper
from kelir.plugin_fusion import FusedTrampoline, RingEvent
from kelir.plugins.plugin_chaos import ChaosConfig
from kelir.plugins.plugin_ledger import LedgerPlugin
from kelir.sdk_errors import EConfig


async def _trap(event: Any) -> Verdict:
    return Verdict.pass_()


async def _watchdog(event: WatchdogEvent) -> WatchdogVerdict:
    return WatchdogVerdict.Mitigate


def test_builder_chain_builds_wrapper() -> None:
    wrapper = PluginBuilder("x").require(LedgerPlugin).on_trap(_trap).build()
    assert wrapper.name == "x"
    assert wrapper.requires == ("ledger",)
    assert wrapper.enabled is True
    descriptors = hook_descriptors(wrapper)
    assert len(descriptors) == 1
    assert descriptors[0].ring is RingEvent.TRAP
    assert descriptors[0].ordinal == 0
    assert descriptors[0].plugin_name == "x"
    builder = KelirBuilder().use(wrapper)
    assert [r.name for r in builder._plugin_registrations] == ["x"]
    assert builder.fuse().hook_count(RingEvent.TRAP) == 1
    assert FusedTrampoline.fuse(wrapper).hook_count(RingEvent.TRAP) == 1


def test_require_missing_raises_econfig_at_boot() -> None:
    wrapper = PluginBuilder("x").require("missing-dep").build()
    builder = KelirBuilder().use(wrapper)
    with pytest.raises(EConfig, match=r"missing prerequisite 'missing-dep'"):
        asyncio.run(builder.boot())


def test_require_satisfied_passes_prerequisite_check() -> None:
    wrapper = PluginBuilder("x").require(LedgerPlugin).build()
    builder = KelirBuilder().use(LedgerPlugin()).use(wrapper)
    with pytest.raises(EConfig) as excinfo:
        asyncio.run(builder.boot())
    assert "missing prerequisite" not in str(excinfo.value)


def test_on_trap_after_build_raises_econfig() -> None:
    builder = PluginBuilder("x").on_trap(_trap)
    builder.build()
    with pytest.raises(EConfig, match=r"already built"):
        builder.on_trap(_trap)
    with pytest.raises(EConfig, match=r"already built"):
        builder.require("ledger")
    with pytest.raises(EConfig, match=r"already built"):
        builder.build()


def test_to_builder_converts_subclass_plugin() -> None:
    class ProbePlugin(BasePlugin):
        name = "probe"

        async def on_trap(self, event: Any) -> Verdict | None:
            return Verdict.pass_()

        async def on_watchdog(self, event: WatchdogEvent) -> WatchdogVerdict:
            return WatchdogVerdict.Mitigate

    wrapper = ProbePlugin().to_builder().build()
    assert wrapper.name == "probe"
    descriptors = hook_descriptors(wrapper)
    assert [d.ring for d in descriptors] == [RingEvent.TRAP, RingEvent.WATCHDOG]
    assert FusedTrampoline.fuse(ProbePlugin()).hook_count(RingEvent.WATCHDOG) == 1


# see: cans/plugins/chaos.md#Network-chaos
def _chaos_net_outbound(config: ChaosConfig, frame: bytearray) -> FilterAction:
    # TODO (L3): apply config.network fault rules to the outbound frame.
    return FilterAction.forward()


# see: cans/plugins/chaos.md#Storage-chaos
def _chaos_fs_write(
    config: ChaosConfig, path: Path, data: Any, pid: int, ctx: HandlerContext
) -> FilterAction:
    # TODO (L3): apply config.fs fault rules to the write.
    return FilterAction.forward()


# see: cans/plugins/chaos.md#Plugin-chaos
def chaos(config: ChaosConfig | None = None) -> _PluginWrapper:
    cfg = config if config is not None else ChaosConfig()
    return (
        PluginBuilder("chaos")
        .on_net_outbound(lambda frame: _chaos_net_outbound(cfg, frame))
        .on_fs_write(
            lambda path, data, pid, ctx: _chaos_fs_write(cfg, path, data, pid, ctx)
        )
        .build()
    )


def test_chaos_factory_builds_wrapper() -> None:
    wrapper = chaos()
    assert wrapper.name == "chaos"
    descriptors = hook_descriptors(wrapper)
    assert [d.ring for d in descriptors] == [
        RingEvent.NET_OUTBOUND,
        RingEvent.FS_WRITE,
    ]
    builder = KelirBuilder().use(wrapper)
    assert builder.fuse().hook_count(RingEvent.NET_OUTBOUND) == 1
    assert builder.fuse().hook_count(RingEvent.FS_WRITE) == 1


def test_vsock_service_scope_and_fork() -> None:
    async def _serve(data: bytes) -> bytes:
        return data

    wrapper = (
        PluginBuilder("svc")
        .scope("guest-only")
        .vsock_service({"kind": 0x0013}, _serve)
        .build()
    )
    assert wrapper.scope == "guest-only"
    descriptors = hook_descriptors(wrapper)
    assert len(descriptors) == 1
    assert descriptors[0].ring is RingEvent.VSOCK_SERVICE
    assert descriptors[0].vsock_kind == 0x0013
    forked = wrapper.fork_state()
    assert forked.name == "svc"
    assert forked is not wrapper
    assert [d.ring for d in hook_descriptors(forked)] == [RingEvent.VSOCK_SERVICE]


def test_fork_with_replays_custom_fork() -> None:
    def _fork(wrapper: Any) -> Any:
        return PluginBuilder(wrapper.name + "-child").on_trap(_trap).build()

    wrapper = PluginBuilder("f").on_trap(_trap).fork_with(_fork).build()
    forked = wrapper.fork_state()
    assert forked.name == "f-child"
    assert [d.ring for d in hook_descriptors(forked)] == [RingEvent.TRAP]


def test_unmapped_ring_raises_econfig_at_build() -> None:
    builder = PluginBuilder("x").on_fs_lock(lambda path: Verdict.pass_())
    with pytest.raises(EConfig, match=r"no Python RingEvent"):
        builder.build()
