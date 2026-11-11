# see: cans/02-plugin-model.md#KelirPlugin-trait
# see: cans/03-sdk-architecture.md#Prerequisite-gating-across-languages
from __future__ import annotations

import asyncio
from pathlib import Path
from typing import Any

import pytest

from kelir import FilterAction, HandlerContext, KelirBuilder, Verdict
from kelir.model_plugin import (
    BasePlugin,
    WatchdogEvent,
    WatchdogVerdict,
    hook_descriptors,
)
from kelir.plugin_fusion import FusedTrampoline, RingEvent
from kelir.plugins.plugin_chaos import ChaosConfig, chaos_builder
from kelir.plugins.plugin_ledger import LedgerPlugin, ledger_builder
from kelir.sdk_errors import EConfig


async def _trap(event: Any) -> Verdict:
    return Verdict.pass_()


async def _watchdog(event: WatchdogEvent) -> WatchdogVerdict:
    return WatchdogVerdict.Mitigate


def _boot_plugin(plugin: KelirBuilder) -> None:
    asyncio.run(KelirBuilder().use(plugin).boot())


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


def test_builder_chain_mounts_plugin() -> None:
    plugin = KelirBuilder.plugin("x").require(LedgerPlugin).on_trap(_trap)
    assert plugin.name == "x"
    assert plugin.requires == ("ledger",)
    assert plugin.enabled is True
    descriptors = hook_descriptors(plugin)
    assert len(descriptors) == 1
    assert descriptors[0].ring is RingEvent.TRAP
    assert descriptors[0].ordinal == 0
    assert descriptors[0].plugin_name == "x"
    builder = KelirBuilder().use(LedgerPlugin()).use(plugin)
    assert [r.name for r in builder._plugin_registrations] == ["ledger", "x"]
    assert builder.fuse().hook_count(RingEvent.TRAP) == 1
    assert FusedTrampoline.fuse(plugin).hook_count(RingEvent.TRAP) == 1


def test_require_missing_raises_econfig_at_mount() -> None:
    plugin = KelirBuilder.plugin("x").require("missing-dep")
    builder = KelirBuilder()
    with pytest.raises(
        EConfig, match=r"missing prerequisite: missing-dep required by x"
    ):
        builder.use(plugin)


def test_host_require_missing_raises_econfig_at_boot() -> None:
    builder = KelirBuilder().require("missing-dep")
    with pytest.raises(
        EConfig, match=r"missing prerequisite: missing-dep required by host"
    ):
        asyncio.run(builder.boot())


def test_require_satisfied_passes_prerequisite_check() -> None:
    plugin = KelirBuilder.plugin("x").require(LedgerPlugin)
    builder = KelirBuilder().use(LedgerPlugin()).use(plugin)
    with pytest.raises(EConfig) as excinfo:
        asyncio.run(builder.boot())
    assert "missing prerequisite" not in str(excinfo.value)


def test_verb_after_mount_raises_econfig() -> None:
    plugin = KelirBuilder.plugin("x").on_trap(_trap)
    KelirBuilder().use(plugin)
    with pytest.raises(EConfig, match=r"plugin already mounted: x"):
        plugin.on_trap(_trap)
    with pytest.raises(EConfig, match=r"plugin already mounted: x"):
        plugin.trap(lambda t: t.on_trap(_trap))
    with pytest.raises(EConfig, match=r"plugin already mounted: x"):
        plugin.require("ledger")
    with pytest.raises(EConfig, match=r"plugin already mounted: x"):
        KelirBuilder().use(plugin)


def test_host_only_verb_rejected_at_mount() -> None:
    plugin = KelirBuilder.plugin("x").terminal(prompt=">")
    with pytest.raises(
        EConfig,
        match=r"x: host-only verb .terminal\(\) called on plugin-mode builder",
    ):
        KelirBuilder().use(plugin)


def test_boot_plugin_mode_raises_econfig() -> None:
    plugin = KelirBuilder.plugin("x")
    with pytest.raises(
        EConfig, match=r"cannot boot a plugin-mode KelirBuilder"
    ):
        asyncio.run(plugin.boot())


def test_use_host_builder_raises_econfig() -> None:
    with pytest.raises(EConfig, match=r"only a plugin-mode KelirBuilder"):
        KelirBuilder().use(KelirBuilder())


def test_to_builder_converts_subclass_plugin() -> None:
    class ProbePlugin(BasePlugin):
        name = "probe"

        async def on_trap(self, event: Any) -> Verdict | None:
            return Verdict.pass_()

        async def on_watchdog(self, event: WatchdogEvent) -> WatchdogVerdict:
            return WatchdogVerdict.Mitigate

    plugin = ProbePlugin().to_builder()
    assert plugin.name == "probe"
    descriptors = hook_descriptors(plugin)
    assert [d.ring for d in descriptors] == [RingEvent.TRAP, RingEvent.WATCHDOG]
    assert FusedTrampoline.fuse(ProbePlugin()).hook_count(RingEvent.WATCHDOG) == 1


def test_all_24_verbs_resolve() -> None:
    plugin = (
        KelirBuilder.plugin("x")
        .on_trap(_trap)
        .on_process_exit(lambda event: None)
        .on_fs_lookup(lambda path: Verdict.pass_())
        .on_fs_read(lambda path, data: data)
        .on_fs_write(lambda path, data, pid, ctx: FilterAction.forward())
        .on_fs_lock(lambda path, fd, op, pid: Verdict.pass_())
        .on_net_outbound(lambda frame: FilterAction.forward())
        .on_net_inbound(lambda frame: FilterAction.forward())
        .on_console_input(lambda port, data: data)
        .on_console_output(lambda port, data: data)
        .on_exec_stdout(lambda pid, exec_id, chunk: chunk)
        .on_exec_stderr(lambda pid, exec_id, chunk: chunk)
        .on_host_lsm_exec(lambda path, pid: Verdict.pass_())
        .on_host_pressure(lambda level: Verdict.pass_())
        .on_host_keyring_access(lambda key: Verdict.pass_())
        .on_vm_init(lambda ctx: None)
        .on_vm_ready(lambda ctx: None)
        .on_vm_shutdown(lambda ctx: None)
        .on_pre_commit(lambda ctx: None)
        .on_post_commit(lambda ctx, committed: None)
        .on_host_preempt(lambda ctx: None)
        .on_memory_pressure(lambda used, total: None)
        .on_heartbeat(lambda tick: None)
        .on_watchdog(_watchdog)
    )
    descriptors = hook_descriptors(plugin)
    assert [d.ordinal for d in descriptors] == list(range(24))
    assert [d.ring for d in descriptors] == [
        RingEvent.TRAP,
        RingEvent.PROCESS_EXIT,
        RingEvent.FS_LOOKUP,
        RingEvent.FS_READ,
        RingEvent.FS_WRITE,
        RingEvent.FS_LOCK,
        RingEvent.NET_OUTBOUND,
        RingEvent.NET_INBOUND,
        RingEvent.CONSOLE_INPUT,
        RingEvent.CONSOLE_OUTPUT,
        RingEvent.EXEC_STDOUT,
        RingEvent.EXEC_STDERR,
        RingEvent.HOST_LSM_EXEC,
        RingEvent.HOST_PRESSURE,
        RingEvent.HOST_KEYRING_ACCESS,
        RingEvent.LIFECYCLE,
        RingEvent.LIFECYCLE,
        RingEvent.LIFECYCLE,
        RingEvent.LIFECYCLE,
        RingEvent.LIFECYCLE,
        RingEvent.HOST_PREEMPT,
        RingEvent.MEMORY_PRESSURE,
        RingEvent.HEARTBEAT,
        RingEvent.WATCHDOG,
    ]
    assert [d.lifecycle_tag for d in descriptors[15:20]] == [
        "vm-init",
        "vm-ready",
        "vm-shutdown",
        "pre-commit",
        "post-commit",
    ]


def test_host_verbs_share_the_24() -> None:
    host = KelirBuilder().on_trap(_trap).on_vm_ready(lambda ctx: None)
    descriptors = host.hook_descriptors()
    assert [d.ring for d in descriptors] == [RingEvent.TRAP, RingEvent.LIFECYCLE]
    assert descriptors[1].lifecycle_tag == "vm-ready"
    assert descriptors[1].plugin_name is None
    assert host.fuse().hook_count(RingEvent.TRAP) == 1


def test_nested_scopes_match_flat_descriptors() -> None:
    nested = (
        KelirBuilder.plugin("x")
        .trap(lambda t: t.on_trap(_trap).on_process_exit(lambda event: None))
        .fs(
            lambda f: f.on_fs_lookup(lambda path: Verdict.pass_())
            .on_fs_read(lambda path, data: data)
            .on_fs_write(lambda path, data, pid, ctx: FilterAction.forward())
            .on_fs_lock(lambda path, fd, op, pid: Verdict.pass_())
        )
        .net(
            lambda n: n.on_net_outbound(lambda frame: FilterAction.forward())
            .on_net_inbound(lambda frame: FilterAction.forward())
        )
        .console(
            lambda c: c.on_console_input(lambda port, data: data)
            .on_console_output(lambda port, data: data)
        )
        .exec(
            lambda e: e.on_exec_stdout(lambda pid, exec_id, chunk: chunk)
            .on_exec_stderr(lambda pid, exec_id, chunk: chunk)
        )
        .host(
            lambda h: h.on_host_lsm_exec(lambda path, pid: Verdict.pass_())
            .on_host_pressure(lambda level: Verdict.pass_())
            .on_host_keyring_access(lambda key: Verdict.pass_())
            .on_host_preempt(lambda ctx: None)
        )
        .lifecycle(
            lambda l: l.on_vm_init(lambda ctx: None)
            .on_vm_ready(lambda ctx: None)
            .on_vm_shutdown(lambda ctx: None)
            .on_pre_commit(lambda ctx: None)
            .on_post_commit(lambda ctx, committed: None)
        )
        .telemetry(
            lambda t: t.on_memory_pressure(lambda used, total: None)
            .on_heartbeat(lambda tick: None)
            .on_watchdog(_watchdog)
        )
    )
    flat = (
        KelirBuilder.plugin("x")
        .on_trap(_trap)
        .on_process_exit(lambda event: None)
        .on_fs_lookup(lambda path: Verdict.pass_())
        .on_fs_read(lambda path, data: data)
        .on_fs_write(lambda path, data, pid, ctx: FilterAction.forward())
        .on_fs_lock(lambda path, fd, op, pid: Verdict.pass_())
        .on_net_outbound(lambda frame: FilterAction.forward())
        .on_net_inbound(lambda frame: FilterAction.forward())
        .on_console_input(lambda port, data: data)
        .on_console_output(lambda port, data: data)
        .on_exec_stdout(lambda pid, exec_id, chunk: chunk)
        .on_exec_stderr(lambda pid, exec_id, chunk: chunk)
        .on_host_lsm_exec(lambda path, pid: Verdict.pass_())
        .on_host_pressure(lambda level: Verdict.pass_())
        .on_host_keyring_access(lambda key: Verdict.pass_())
        .on_host_preempt(lambda ctx: None)
        .on_vm_init(lambda ctx: None)
        .on_vm_ready(lambda ctx: None)
        .on_vm_shutdown(lambda ctx: None)
        .on_pre_commit(lambda ctx: None)
        .on_post_commit(lambda ctx, committed: None)
        .on_memory_pressure(lambda used, total: None)
        .on_heartbeat(lambda tick: None)
        .on_watchdog(_watchdog)
    )
    nested_keys = [
        (d.ring, d.lifecycle_tag, d.ordinal)
        for d in hook_descriptors(nested)
    ]
    flat_keys = [
        (d.ring, d.lifecycle_tag, d.ordinal) for d in hook_descriptors(flat)
    ]
    assert nested_keys == flat_keys


def test_scope_rejects_out_of_group_verb() -> None:
    plugin = KelirBuilder.plugin("x")
    with pytest.raises(EConfig, match=r"not in this scope"):
        plugin.trap(lambda t: t.on_fs_write(lambda p, d, pid, c: None))


def test_append_order_duplicate_handlers() -> None:
    def _first(event: Any) -> Verdict:
        return Verdict.pass_()

    def _second(event: Any) -> Verdict:
        return Verdict.pass_()

    plugin = KelirBuilder.plugin("x").on_trap(_first).on_trap(_second)
    descriptors = hook_descriptors(plugin)
    assert len(descriptors) == 2
    assert [d.ring for d in descriptors] == [RingEvent.TRAP, RingEvent.TRAP]
    assert [d.ordinal for d in descriptors] == [0, 1]
    assert descriptors[0].run is _first
    assert descriptors[1].run is _second
    assert FusedTrampoline.fuse(plugin).hook_count(RingEvent.TRAP) == 2


def test_multi_vsock_registers_each_service() -> None:
    async def _serve(data: bytes) -> bytes:
        return data

    plugin = (
        KelirBuilder.plugin("svc")
        .vsock_service({"port": 5252, "service": "one"}, _serve)
        .vsock_service({"port": 5253, "service": "two"}, _serve)
    )
    descriptors = hook_descriptors(plugin)
    assert len(descriptors) == 2
    assert [d.ring for d in descriptors] == [
        RingEvent.VSOCK_SERVICE,
        RingEvent.VSOCK_SERVICE,
    ]
    assert [d.vsock_kind for d in descriptors] == [5252, 5253]
    assert [d.ordinal for d in descriptors] == [0, 1]


def test_vsock_validation_errors() -> None:
    async def _serve(data: bytes) -> bytes:
        return data

    with pytest.raises(EConfig, match=r"invalid port 0"):
        _boot_plugin(
            KelirBuilder.plugin("x").vsock_service(
                {"port": 0, "service": "s"}, _serve
            )
        )
    with pytest.raises(EConfig, match=r"invalid port 65536"):
        _boot_plugin(
            KelirBuilder.plugin("x").vsock_service(
                {"port": 65536, "service": "s"}, _serve
            )
        )
    with pytest.raises(EConfig, match=r"no service name"):
        _boot_plugin(
            KelirBuilder.plugin("x").vsock_service({"port": 5252}, _serve)
        )
    with pytest.raises(EConfig, match=r"invalid port None"):
        _boot_plugin(
            KelirBuilder.plugin("x").vsock_service({"service": "s"}, _serve)
        )


def test_scope_validation_errors() -> None:
    with pytest.raises(EConfig, match=r"unknown MountScope keys"):
        _boot_plugin(
            KelirBuilder.plugin("x").scope(
                {"paths": ["/workspace"], "nope": []}
            )
        )
    with pytest.raises(EConfig, match=r"must be a MountScope mapping"):
        _boot_plugin(KelirBuilder.plugin("x").scope("guest-only"))
    with pytest.raises(EConfig, match=r"must be a list"):
        _boot_plugin(KelirBuilder.plugin("x").scope({"paths": "/workspace"}))


def test_arity_econfig() -> None:
    with pytest.raises(EConfig, match=r"takes 1 positional"):
        KelirBuilder.plugin("x").on_fs_write(lambda path: None)
    with pytest.raises(EConfig, match=r"takes 3 positional"):
        KelirBuilder.plugin("x").on_trap(lambda a, b, c: None)
    plugin = (
        KelirBuilder.plugin("x")
        .on_trap(lambda event: Verdict.pass_())
        .on_trap(lambda event, ctx: Verdict.pass_())
    )
    assert len(hook_descriptors(plugin)) == 2


def test_canonical_chaos_shape() -> None:
    cfg = ChaosConfig()
    plugin = (
        KelirBuilder.plugin("chaos")
        .require("ledger")
        .context(lambda: cfg)
        .scope({"paths": ["/workspace"]})
        .fs(
            lambda f: f.on_fs_write(
                lambda path, data, pid, ctx: _chaos_fs_write(cfg, path, data, pid, ctx)
            )
        )
        .net(
            lambda n: n.on_net_outbound(
                lambda frame: _chaos_net_outbound(cfg, frame)
            )
        )
    )
    assert plugin.name == "chaos"
    assert plugin.requires == ("ledger",)
    assert plugin._scope == {"paths": ["/workspace"]}
    assert plugin._context_fn is not None
    assert plugin._context_fn() is cfg
    descriptors = hook_descriptors(plugin)
    assert [d.ring for d in descriptors] == [
        RingEvent.FS_WRITE,
        RingEvent.NET_OUTBOUND,
    ]
    builder = KelirBuilder().use(LedgerPlugin()).use(plugin)
    assert builder.fuse().hook_count(RingEvent.NET_OUTBOUND) == 1
    assert builder.fuse().hook_count(RingEvent.FS_WRITE) == 1


def test_factory_builders_use_unified_shape() -> None:
    chaos = chaos_builder()
    assert isinstance(chaos, KelirBuilder)
    assert chaos.name == "chaos"
    assert chaos._mode == "plugin"
    assert chaos._context_fn is not None
    assert isinstance(chaos._context_fn(), ChaosConfig)
    ledger = ledger_builder()
    assert isinstance(ledger, KelirBuilder)
    assert ledger.name == "ledger"
    assert ledger._mode == "plugin"


def test_vsock_service_scope_and_fork() -> None:
    async def _serve(data: bytes) -> bytes:
        return data

    plugin = (
        KelirBuilder.plugin("svc")
        .scope({"paths": ["/workspace"]})
        .vsock_service({"kind": 0x0013, "service": "svc"}, _serve)
    )
    assert plugin._scope == {"paths": ["/workspace"]}
    descriptors = hook_descriptors(plugin)
    assert len(descriptors) == 1
    assert descriptors[0].ring is RingEvent.VSOCK_SERVICE
    assert descriptors[0].vsock_kind == 0x0013
    forked = plugin.fork_state()
    assert forked.name == "svc"
    assert forked is not plugin
    assert [d.ring for d in hook_descriptors(forked)] == [RingEvent.VSOCK_SERVICE]


def test_fork_with_replays_custom_fork() -> None:
    def _fork(plugin: KelirBuilder) -> KelirBuilder:
        return KelirBuilder.plugin(f"{plugin.name}-child").on_trap(_trap)

    plugin = KelirBuilder.plugin("f").on_trap(_trap).fork_with(_fork)
    forked = plugin.fork_state()
    assert forked.name == "f-child"
    assert [d.ring for d in hook_descriptors(forked)] == [RingEvent.TRAP]


def test_fork_state_on_host_raises_econfig() -> None:
    with pytest.raises(EConfig, match=r"Session.fork"):
        KelirBuilder().fork_state()
