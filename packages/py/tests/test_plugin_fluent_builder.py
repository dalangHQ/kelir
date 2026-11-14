# see: cans/02-plugin-model.md#KelirPlugin-trait
# see: cans/03-sdk-architecture.md#Prerequisite-gating-across-languages
from __future__ import annotations

import asyncio
from pathlib import Path
from typing import Any

import pytest

from kelir import FilterAction, HandlerContext, KelirBuilder, TrapEvent, TrapFilter, Verdict
from kelir.plugin_model import (
    BasePlugin,
    WatchdogEvent,
    WatchdogVerdict,
    hook_descriptors,
)
from kelir.plugin_fusion import FusedTrampoline, RingEvent
from kelir.plugins.plugin_chaos import ChaosConfig, chaos_builder
from kelir.plugins.plugin_ledger import LedgerPlugin, ledger_builder
from kelir.sdk_errors import EConfig


async def _trap(event: TrapEvent) -> Verdict:
    return Verdict.pass_()


async def _watchdog(event: WatchdogEvent) -> WatchdogVerdict:
    return WatchdogVerdict.Mitigate


def _boot_plugin(plugin: KelirBuilder) -> None:
    asyncio.run(KelirBuilder().use(plugin).boot())


# see: cans/plugins/chaos.md#Network-chaos
def _chaos_net_outbound(config: ChaosConfig, frame: bytearray) -> FilterAction:
    # TODO (L3) [pe_16_chaos_rst_injection_drops_matching_frame]: apply config.network fault rules to the outbound frame.; slop: network fault rule never applied, RST drop never fires
    return FilterAction.forward()


# see: cans/plugins/chaos.md#Storage-chaos
def _chaos_fs_write(
    config: ChaosConfig,
    path: Path,
    data: bytes,
    pid: int,
    ctx: HandlerContext[None, None],
) -> FilterAction:
    # TODO (L3) [plugin_engine]: apply config.fs fault rules to the write.; slop: fs fault rule never applied, fault drill passes vacuously
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


def test_require_missing_raises_econfig_at_seal() -> None:
    plugin = KelirBuilder.plugin("x").require("missing-dep")
    builder = KelirBuilder().use(plugin)
    with pytest.raises(
        EConfig, match=r"missing prerequisite: missing-dep required by x"
    ):
        builder.seal()


def test_require_satisfied_out_of_order_mount_passes_at_seal() -> None:
    plugin = KelirBuilder.plugin("x").require(LedgerPlugin)
    builder = KelirBuilder().use(plugin).use(LedgerPlugin())
    assert builder.seal() is builder


def test_duplicate_mount_raises_econfig_at_seal() -> None:
    plugin = KelirBuilder.plugin("x").on_trap(_trap)
    builder = KelirBuilder().use(plugin).use(plugin)
    with pytest.raises(EConfig, match=r"plugin already mounted: x"):
        builder.seal()
    other = KelirBuilder.plugin("x").on_trap(_trap)
    twin = KelirBuilder().use(KelirBuilder.plugin("x")).use(other)
    with pytest.raises(EConfig, match=r"plugin already mounted: x"):
        twin.seal()
    base = KelirBuilder().use(LedgerPlugin()).use(LedgerPlugin())
    with pytest.raises(EConfig, match=r"plugin already mounted: ledger"):
        base.seal()


def test_host_require_missing_raises_econfig_at_boot() -> None:
    builder = KelirBuilder().require("missing-dep")
    with pytest.raises(
        EConfig, match=r"missing prerequisite: missing-dep required by host"
    ):
        asyncio.run(builder.boot())


def test_require_satisfied_passes_prerequisite_check() -> None:
    plugin = KelirBuilder.plugin("x").require(LedgerPlugin)
    builder = KelirBuilder().use(LedgerPlugin()).use(plugin)
    with pytest.raises((EConfig, ValueError)) as excinfo:
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
        plugin.trap(TrapFilter.binary("x"), pre=_trap)
    with pytest.raises(EConfig, match=r"plugin already mounted: x"):
        plugin.require("ledger")
    with pytest.raises(EConfig, match=r"plugin already mounted: x"):
        plugin.group(lambda b: b)
    host = KelirBuilder().use(plugin)
    with pytest.raises(EConfig, match=r"plugin already mounted: x"):
        host.seal()


def test_host_only_verbs_raise_at_call_site() -> None:
    async def _echo(request: bytes) -> bytes:
        return request

    async def _quarantine(payload: dict[str, Any]) -> None:
        return None

    plugin = KelirBuilder.plugin("x")
    with pytest.raises(
        EConfig,
        match=r"x: host-only verb \.detached\(\) called on plugin-mode builder",
    ):
        plugin.detached(True)
    with pytest.raises(
        EConfig,
        match=r"x: host-only verb \.terminal\(\) called on plugin-mode builder",
    ):
        plugin.terminal(prompt=">")
    with pytest.raises(
        EConfig,
        match=r"x: host-only verb \.storage\(\) called on plugin-mode builder",
    ):
        plugin.storage(lock_broker=True)
    with pytest.raises(
        EConfig,
        match=r"x: host-only verb \.wire\(\) called on plugin-mode builder",
    ):
        plugin.wire(sinkhole=None)
    with pytest.raises(
        EConfig,
        match=r"x: host-only verb \.params\(\) called on plugin-mode builder",
    ):
        plugin.params(dict)
    # see: cans/02-plugin-model.md#Chain-decisions
    before = len(plugin.hook_descriptors())
    plugin.trap(TrapFilter.binary("x"), pre=_trap)
    assert len(plugin.hook_descriptors()) == before + 1
    with pytest.raises(
        EConfig,
        match=r"x: host-only verb \.register_custom_action\(\) called on plugin-mode builder",
    ):
        plugin.register_custom_action("q", {"type": "object"}, _quarantine)
    with pytest.raises(
        EConfig,
        match=r"x: host-only verb \.register_stream_kind\(\) called on plugin-mode builder",
    ):
        plugin.register_stream_kind(0x0013, {"type": "object"}, _echo)
    with pytest.raises(
        EConfig,
        match=r"x: host-only verb \.on_trap_deny\(\) called on plugin-mode builder",
    ):
        plugin.on_trap_deny(lambda event, session: None)


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
    class ProbePlugin(BasePlugin[None, None]):
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


def test_scope_end_returns_builder() -> None:
    plugin = KelirBuilder.plugin("x")
    seen: list[KelirBuilder] = []
    plugin.trap(lambda t: seen.append(t.on_trap(_trap).end()))
    assert seen[0] is plugin
    chained = KelirBuilder.plugin("y").trap(
        lambda t: t.on_trap(_trap).end().on_process_exit(lambda event: None)
    )
    assert [d.ring for d in hook_descriptors(chained)] == [
        RingEvent.TRAP,
        RingEvent.PROCESS_EXIT,
    ]


def test_scope_callback_may_return_none() -> None:
    def _configure(scope: Any) -> None:
        scope.on_trap(_trap)

    plugin = KelirBuilder.plugin("x").trap(_configure)
    assert len(hook_descriptors(plugin)) == 1
    assert hook_descriptors(plugin)[0].ring is RingEvent.TRAP


def test_trap_scope_on_trap_deny() -> None:
    host = KelirBuilder().trap(
        lambda t: t.on_trap(_trap).on_trap_deny(lambda event, session: None)
    )
    descriptors = host.hook_descriptors()
    assert [d.lifecycle_tag for d in descriptors] == [None, "trap-deny"]
    assert descriptors[1].ring is RingEvent.LIFECYCLE
    with pytest.raises(EConfig, match=r"host-only verb \.on_trap_deny\(\)"):
        KelirBuilder.plugin("x").trap(
            lambda t: t.on_trap_deny(lambda event, session: None)
        )


def test_plugin_instance_call_mounts_builder() -> None:
    host = KelirBuilder().plugin(KelirBuilder.plugin("chaos"))
    assert host is not None
    assert [r.name for r in host._plugin_registrations] == ["chaos"]
    assert KelirBuilder.plugin("chaos").name == "chaos"
    with pytest.raises(EConfig, match=r"only a plugin-mode KelirBuilder"):
        KelirBuilder().plugin(KelirBuilder())


def test_plugin_instance_call_with_name_raises_clear_econfig() -> None:
    with pytest.raises(
        EConfig, match=r"mounts a plugin-mode builder, not a name"
    ):
        KelirBuilder().plugin("chaos")
    with pytest.raises(EConfig) as excinfo:
        KelirBuilder().plugin("chaos")
    assert "expected a BasePlugin" not in str(excinfo.value)
    assert "KelirBuilder.plugin('chaos')" in str(excinfo.value)


def test_plugin_instance_call_with_base_plugin_raises_clear_econfig() -> None:
    with pytest.raises(
        EConfig, match=r"mounts only a plugin-mode KelirBuilder"
    ):
        KelirBuilder().plugin(LedgerPlugin())


def test_group_runs_block_on_same_builder() -> None:
    plugin = KelirBuilder.plugin("x").group(lambda b: b.on_trap(_trap))
    assert [d.ring for d in hook_descriptors(plugin)] == [RingEvent.TRAP]
    assert plugin.group(lambda b: b) is plugin
    assert plugin.group(lambda b: None) is plugin
    host = KelirBuilder().group(lambda b: b.on_trap(_trap))
    assert [d.ring for d in host.hook_descriptors()] == [RingEvent.TRAP]
    with pytest.raises(
        EConfig, match=r"group\(\) must return the builder it receives"
    ):
        KelirBuilder.plugin("y").group(lambda b: KelirBuilder.plugin("z"))


def test_guard_applies_only_when_enabled() -> None:
    skipped = KelirBuilder.plugin("x").guard(False, lambda b: b.on_trap(_trap))
    assert hook_descriptors(skipped) == []
    applied = KelirBuilder.plugin("y").guard(True, lambda b: b.on_trap(_trap))
    assert len(hook_descriptors(applied)) == 1
    chained = (
        KelirBuilder.plugin("z")
        .group(lambda b: b.on_trap(_trap))
        .guard(True, lambda b: b.on_process_exit(lambda event: None))
        .guard(False, lambda b: b.on_fs_lookup(lambda path: Verdict.pass_()))
    )
    assert [d.ring for d in hook_descriptors(chained)] == [
        RingEvent.TRAP,
        RingEvent.PROCESS_EXIT,
    ]
    with pytest.raises(EConfig, match=r"expects a bool"):
        KelirBuilder.plugin("w").guard(1, lambda b: b.on_trap(_trap))  # type: ignore[arg-type]


def test_context_on_host_raises_econfig() -> None:
    with pytest.raises(EConfig, match=r"\.context\(\) is plugin-only"):
        KelirBuilder().context(lambda: {})
    plugin = KelirBuilder.plugin("x").context(lambda: {"n": 1})
    assert plugin._context_fn is not None
    assert plugin._context_fn() == {"n": 1}


def test_seal_validates_and_chains() -> None:
    host = KelirBuilder().on_trap(_trap)
    assert host.seal() is host
    assert host.seal().on_vm_ready(lambda ctx: None) is host
    with pytest.raises(
        EConfig, match=r"missing prerequisite: missing-dep required by host"
    ):
        KelirBuilder().require("missing-dep").seal()


def test_seal_validates_plugin_shape_without_boot() -> None:
    async def _serve(data: bytes) -> bytes:
        return data

    plugin = KelirBuilder.plugin("x").vsock_service(
        {"port": 0, "service": "s"}, _serve
    )
    with pytest.raises(EConfig, match=r"invalid port 0"):
        plugin.seal()


def test_lock_broker_verb() -> None:
    host = KelirBuilder().lock_broker()
    assert host._config["storage"]["lock_broker"] is True
    assert host.lock_broker(False)._config["storage"]["lock_broker"] is False
    merged = KelirBuilder().storage(shared_caches=["/x"]).lock_broker()
    assert merged._config["storage"] == {
        "shared_caches": ["/x"],
        "lock_broker": True,
    }
    kept = KelirBuilder().lock_broker().storage(shared_caches=["/x"])
    assert kept._config["storage"]["lock_broker"] is True
    with pytest.raises(EConfig, match=r"host-only verb \.lock_broker\(\)"):
        KelirBuilder.plugin("x").lock_broker()
    with pytest.raises(EConfig, match=r"expects a bool"):
        KelirBuilder().lock_broker("yes")  # type: ignore[arg-type]


def test_fork_state_is_factory() -> None:
    plugin = KelirBuilder.plugin("svc").on_trap(_trap)
    forked = plugin.fork_state()
    assert forked is not plugin
    assert forked._mode == "plugin"
    assert forked.name == "svc"
    forked.on_process_exit(lambda event: None)
    assert len(hook_descriptors(plugin)) == 1
    assert len(hook_descriptors(forked)) == 2
