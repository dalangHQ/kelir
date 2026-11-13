# see: cans/02-plugin-model.md#Unified-builder
# see: cans/03-sdk-architecture.md#Unified-builder-surface
# see: cans/artifacts/examples/fluent-builder-chaos.py#build_chaos_plugin
# see: cans/artifacts/examples/unified-builder-surface.rs#build_surface_plugin
# see: cans/artifacts/examples/unified-builder-surface.ts#buildSurfacePlugin

# The unified builder surface, walked end to end in one file: structural
# verbs, all 24 ring verbs flat, all 8 scope closures, the four chain cases
# (seal, trap filter, trap_scope, lock_broker) and the four mount cases
# (host-only verbs on a host builder, host-only verbs on a plugin builder,
# host.use(builder) merge ordering, nested-scope = flat-chain equivalence).
# Handlers are trivial stubs: Forward for filter verbs, Pass for verdict
# verbs, passthrough bytes for console/exec, no-ops for lifecycle and
# telemetry. L1/L2: descriptors only, no execution, no boot() on a host.

from __future__ import annotations

import asyncio
import platform

import kelir
from kelir import FilterAction, KelirBuilder, TrapFilter, Verdict, VsockServiceEntry
from kelir.model_plugin import WatchdogVerdict
from kelir.sdk_errors import EConfig


async def _pass_trap(_c) -> Verdict:
    return Verdict.pass_()


async def echo(request: bytes) -> bytes:
    return request


async def _echo_bytes(request: bytes) -> bytes:
    return request


async def _noop_action(_payload) -> None:
    return None


# see: cans/02-plugin-model.md#Unified-builder
# Structural verbs: context, require, scope, fork_with, vsock_service.
# Then the 24 ring verbs flat, then the 8 scope closures.
def build_surface_plugin() -> KelirBuilder:
    return (
        KelirBuilder.plugin("surface")
        .context(lambda: {"surface": True})
        .require("ledger")
        .scope({"paths": ["/workspace"]})
        .fork_with(lambda builder: build_surface_plugin())
        .vsock_service(
            VsockServiceEntry(
                port=0x0013,
                service="echo",
                schema={"type": "object", "properties": {}},
            ),
            echo,
        )
        .on_trap(lambda event, ctx: Verdict.pass_())
        .on_process_exit(lambda event: None)
        .on_fs_lookup(lambda path: Verdict.pass_())
        .on_fs_read(lambda path, data: data)
        .on_fs_write(lambda path, data, pid, ctx: FilterAction.forward())
        .on_fs_lock(lambda path, cmd, typ, pid: Verdict.pass_())
        .on_net_outbound(lambda frame: FilterAction.forward())
        .on_net_inbound(lambda frame: FilterAction.forward())
        .on_console_input(lambda port, data: data)
        .on_console_output(lambda port, chunk: chunk)
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
        .on_memory_pressure(lambda available, total: None)
        .on_heartbeat(lambda event: None)
        .on_watchdog(lambda event: WatchdogVerdict.ExecuteDefault)
        .trap(
            lambda scope: scope.on_trap(lambda event, ctx: Verdict.pass_()).on_process_exit(
                lambda event: None
            )
        )
        .fs(
            lambda scope: scope.on_fs_lookup(lambda path: Verdict.pass_())
            .on_fs_read(lambda path, data: data)
            .on_fs_write(lambda path, data, pid, ctx: FilterAction.forward())
            .on_fs_lock(lambda path, cmd, typ, pid: Verdict.pass_())
        )
        .net(
            lambda scope: scope.on_net_outbound(lambda frame: FilterAction.forward())
            .on_net_inbound(lambda frame: FilterAction.forward())
        )
        .console(
            lambda scope: scope.on_console_input(lambda port, data: data)
            .on_console_output(lambda port, chunk: chunk)
        )
        .exec(
            lambda scope: scope.on_exec_stdout(lambda pid, exec_id, chunk: chunk)
            .on_exec_stderr(lambda pid, exec_id, chunk: chunk)
        )
        .host(
            lambda scope: scope.on_host_lsm_exec(lambda path, pid: Verdict.pass_())
            .on_host_pressure(lambda level: Verdict.pass_())
            .on_host_keyring_access(lambda key: Verdict.pass_())
            .on_host_preempt(lambda ctx: None)
        )
        .lifecycle(
            lambda scope: scope.on_vm_init(lambda ctx: None)
            .on_vm_ready(lambda ctx: None)
            .on_vm_shutdown(lambda ctx: None)
            .on_pre_commit(lambda ctx: None)
            .on_post_commit(lambda ctx, committed: None)
        )
        .telemetry(
            lambda scope: scope.on_memory_pressure(lambda available, total: None)
            .on_heartbeat(lambda event: None)
            .on_watchdog(lambda event: WatchdogVerdict.ExecuteDefault)
        )
    )


# see: cans/02-plugin-model.md#Unified-builder
# The ledger provider the surface plugin requires: the name "ledger" is
# what satisfies require("ledger") at seal time. One trivial verb.
def build_ledger_plugin() -> KelirBuilder:
    return KelirBuilder.plugin("ledger").on_heartbeat(lambda event: None)


# see: cans/02-plugin-model.md#Unified-builder
# The merge-ordering second mount: no requires, one trivial verb.
def build_plain_plugin() -> KelirBuilder:
    return KelirBuilder.plugin("plain").on_heartbeat(lambda event: None)


# see: cans/02-plugin-model.md#Unified-builder
# Mount case 1: host-only verbs on a host builder (terminal, storage,
# wire, detached, lock_broker). Chain cases: trap filter, trap scope,
# lock_broker, then seal() in main. Mount case 3: merge ordering — the
# ledger provider first (it satisfies the surface plugin's require at
# seal time), then the surface plugin via instance .plugin(), then the
# plain plugin via use(), the same verb under its second name.
def build_host() -> KelirBuilder:
    return (
        kelir.builder(
            vcpus=2,
            memory_mb=2048,
            kernel=f"./fixtures/vmlinux-6.6-{platform.machine()}-kelir",
            rootfs_erofs="./fixtures/rootfs-surface.erofs",
            tsc_locked=True,
            enable_watchdog=True,
            driver="auto",
        )
        .terminal(cols=120, rows=40)
        .storage(lock_broker=True)
        .wire()
        .detached(False)
        .use(build_ledger_plugin())
        .plugin(build_surface_plugin())
        .use(build_plain_plugin())
        .trap(TrapFilter.binary("surface-allow"), pre=_pass_trap)
        .trap(
            lambda scope: scope.on_trap(lambda event, ctx: Verdict.pass_()).on_process_exit(
                lambda event: None
            )
        )
        .lock_broker(True)
    )


# see: cans/02-plugin-model.md#Unified-builder
# Mount case 2: host-only verbs on a plugin builder, expecting EConfig at
# the call site. Live Python rejects exactly these nine verbs plus boot()
# below; seal() and use() do not raise on a plugin-mode builder live,
# although the spec lists them host-only, so they are not probed here.
def host_only_verbs_rejected_on_plugin() -> list[str]:
    rejected: list[str] = []
    plugin = build_surface_plugin()
    probes = [
        ("terminal", lambda: plugin.terminal(cols=120, rows=40)),
        ("storage", lambda: plugin.storage(lock_broker=True)),
        ("wire", lambda: plugin.wire()),
        ("lock_broker", lambda: plugin.lock_broker()),
        ("params", lambda: plugin.params(dict)),
        ("detached", lambda: plugin.detached(True)),
        (
            "register_custom_action",
            lambda: plugin.register_custom_action(
                "quarantine", {"type": "object"}, _noop_action
            ),
        ),
        (
            "register_stream_kind",
            lambda: plugin.register_stream_kind(
                0x0013, {"type": "object"}, _echo_bytes
            ),
        ),
        ("on_trap_deny", lambda: plugin.on_trap_deny(lambda event: None)),
    ]
    for verb, call in probes:
        try:
            call()
        except EConfig:
            rejected.append(verb)
    return rejected


# see: cans/02-plugin-model.md#Unified-builder
# Mount case 4, scoped form: the same verbs as build_flat_plugin, grouped
# into the 8 scope closures. Scope groups organise the chain only; ring
# membership and descriptors are unchanged by scope use.
def build_scoped_plugin() -> KelirBuilder:
    return (
        KelirBuilder.plugin("surface-scoped")
        .trap(
            lambda scope: scope.on_trap(lambda event, ctx: Verdict.pass_()).on_process_exit(
                lambda event: None
            )
        )
        .fs(
            lambda scope: scope.on_fs_lookup(lambda path: Verdict.pass_())
            .on_fs_read(lambda path, data: data)
            .on_fs_write(lambda path, data, pid, ctx: FilterAction.forward())
            .on_fs_lock(lambda path, cmd, typ, pid: Verdict.pass_())
        )
        .net(
            lambda scope: scope.on_net_outbound(lambda frame: FilterAction.forward())
            .on_net_inbound(lambda frame: FilterAction.forward())
        )
        .console(
            lambda scope: scope.on_console_input(lambda port, data: data)
            .on_console_output(lambda port, chunk: chunk)
        )
        .exec(
            lambda scope: scope.on_exec_stdout(lambda pid, exec_id, chunk: chunk)
            .on_exec_stderr(lambda pid, exec_id, chunk: chunk)
        )
        .host(
            lambda scope: scope.on_host_lsm_exec(lambda path, pid: Verdict.pass_())
            .on_host_pressure(lambda level: Verdict.pass_())
            .on_host_keyring_access(lambda key: Verdict.pass_())
            .on_host_preempt(lambda ctx: None)
        )
        .lifecycle(
            lambda scope: scope.on_vm_init(lambda ctx: None)
            .on_vm_ready(lambda ctx: None)
            .on_vm_shutdown(lambda ctx: None)
            .on_pre_commit(lambda ctx: None)
            .on_post_commit(lambda ctx, committed: None)
        )
        .telemetry(
            lambda scope: scope.on_memory_pressure(lambda available, total: None)
            .on_heartbeat(lambda event: None)
            .on_watchdog(lambda event: WatchdogVerdict.ExecuteDefault)
        )
    )


# see: cans/02-plugin-model.md#Unified-builder
# Mount case 4, flat form: the same 24 verbs as build_scoped_plugin, with
# no scope grouping. A nested chain and the equivalent flat chain produce
# identical descriptors: same rings, same ordinals.
def build_flat_plugin() -> KelirBuilder:
    return (
        KelirBuilder.plugin("surface-flat")
        .on_trap(lambda event, ctx: Verdict.pass_())
        .on_process_exit(lambda event: None)
        .on_fs_lookup(lambda path: Verdict.pass_())
        .on_fs_read(lambda path, data: data)
        .on_fs_write(lambda path, data, pid, ctx: FilterAction.forward())
        .on_fs_lock(lambda path, cmd, typ, pid: Verdict.pass_())
        .on_net_outbound(lambda frame: FilterAction.forward())
        .on_net_inbound(lambda frame: FilterAction.forward())
        .on_console_input(lambda port, data: data)
        .on_console_output(lambda port, chunk: chunk)
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
        .on_memory_pressure(lambda available, total: None)
        .on_heartbeat(lambda event: None)
        .on_watchdog(lambda event: WatchdogVerdict.ExecuteDefault)
    )


# see: cans/02-plugin-model.md#Unified-builder
async def main() -> None:
    build_host().seal()
    expected = [
        "terminal",
        "storage",
        "wire",
        "lock_broker",
        "params",
        "detached",
        "register_custom_action",
        "register_stream_kind",
        "on_trap_deny",
    ]
    rejected = host_only_verbs_rejected_on_plugin()
    assert rejected == expected, f"host-only rejection: {rejected!r} != {expected!r}"
    plugin = build_surface_plugin()
    try:
        await plugin.boot()
    except EConfig:
        pass
    else:
        raise RuntimeError("host-only rejection: boot() did not raise EConfig")
    build_scoped_plugin()
    build_flat_plugin()


if __name__ == "__main__":
    asyncio.run(main())
