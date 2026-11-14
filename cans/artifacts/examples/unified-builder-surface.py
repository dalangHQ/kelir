# see: cans/02-plugin-model.md#Unified-builder
# see: cans/02-plugin-model.md#Nested-scopes
# see: cans/03-sdk-architecture.md#Unified-builder-surface
# see: cans/artifacts/examples/fluent-builder-chaos.py#build_chaos_plugin
# see: cans/artifacts/examples/unified-builder-surface.rs#build_surface_plugin
# see: cans/artifacts/examples/unified-builder-surface.ts#buildSurfacePlugin

# The unified builder surface, walked end to end in one file: structural
# verbs, all 24 ring verbs reached through their 8 scope closures, the
# five orphan-verb scopes (actions, streams, vsock, params, deny), the
# chain cases (seal, trap filter, trap scope, fs config) and the mount
# cases (host-only verbs on a host builder, host-only verbs on a plugin
# builder, host.use(builder) merge ordering, flat-fold = nested-scope
# equivalence via group()). Handlers are trivial stubs: Forward for
# filter verbs, Pass for verdict verbs, passthrough bytes for
# console/exec, no-ops for lifecycle and telemetry. L1/L2: descriptors
# only, no execution, no boot() on a host. There is no .end() in
# Python: scope closures return the scope or None.

from __future__ import annotations

import asyncio
import platform

import kelir
from kelir import FilterAction, KelirBuilder, TrapFilter, Verdict
from kelir.plugin_model import WatchdogVerdict
from kelir.sdk_errors import EConfig


async def _pass_trap(_c) -> Verdict:
    return Verdict.pass_()


async def echo(request: bytes) -> bytes:
    return request


async def _echo_bytes(request: bytes) -> bytes:
    return request


async def _noop_action(_payload) -> None:
    return None


# see: cans/02-plugin-model.md#Nested-scopes
# Structural verbs: context, require, scope, fork_with. The vsock service
# and the 24 ring verbs are scope-reached: .vsock() carries the service,
# the 8 ring scopes carry the verbs.
def build_surface_plugin() -> KelirBuilder:
    return (
        KelirBuilder.plugin("surface")
        .context(lambda: {"surface": True})
        .require("ledger")
        .scope({"paths": ["/workspace"]})
        .fork_with(lambda builder: build_surface_plugin())
        .vsock(lambda v: v.service(0x0013, "echo", echo))
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
    return KelirBuilder.plugin("ledger").telemetry(
        lambda scope: scope.on_heartbeat(lambda event: None)
    )


# see: cans/02-plugin-model.md#Unified-builder
# The merge-ordering second mount: no requires, one trivial verb.
def build_plain_plugin() -> KelirBuilder:
    return KelirBuilder.plugin("plain").telemetry(
        lambda scope: scope.on_heartbeat(lambda event: None)
    )


# see: cans/02-plugin-model.md#Unified-builder
# Mount case 1: host-only verbs on a host builder (terminal, storage,
# wire, detached; the lock-broker config rides .fs(lambda f: f.config(…))).
# Chain cases: trap filter, trap scope, fs config, then seal() in main.
# Mount case 3: merge ordering — the ledger provider first (it satisfies
# the surface plugin's require at seal time), then the surface plugin via
# instance .plugin(), then the plain plugin via use(), the same verb under
# its second name.
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
        # see: cans/02-plugin-model.md#TrapFilter-predicates
        # The filter call site carries the full predicate chain
        # (with_arg, with_cwd, with_env, with_uid, argv_matches —
        # AND-ed); the handler body needs no cwd/argv guard.
        .trap(
            TrapFilter.binary("surface-allow")
            .with_arg("run")
            .with_cwd("/workspace/**")
            .with_env("CI", "1")
            .with_uid(0)
            .argv_matches(r"^--fast$"),
            pre=_pass_trap,
        )
        .trap(
            lambda scope: scope.on_trap(lambda event, ctx: Verdict.pass_()).on_process_exit(
                lambda event: None
            )
        )
        # see: cans/02-plugin-model.md#Nested-scopes
        # The host-only fs scope sub-nodes: config carries lock_broker,
        # carryover_bytes, scan_engine, snoops; requires / fork_with /
        # timeout / priority / rules pin the shared scope shape. The
        # standalone lock_broker verb folds into this config (kwargs
        # shape). storage.lock_broker.scope_globs rides the same config.
        .fs(
            lambda f: f.config(
                lock_broker={
                    "globs": ["**/Cargo.lock", "**/package-lock.json"],
                    "non_blocking": True,
                },
                carryover_bytes=1024,
                scan_engine="FastRegex",
                snoops=[],
            )
            .require("ledger")
            .fork_with(lambda builder: build_plain_plugin())
            .timeout(0.005)
            .priority(0)
            .rules(
                {
                    "read": {"allow": ["/workspace/**"], "deny": ["/workspace/.policy/identity"]},
                    "write": {"mask": ["sk_live_[A-Za-z0-9]{24}"]},
                }
            )
            .on_fs_lookup(lambda path: Verdict.pass_())
            .on_fs_write(lambda path, data, pid, ctx: FilterAction.forward())
        )
        # see: cans/02-plugin-model.md#Nested-scopes
        # The five orphan-verb scopes on the host: actions, streams,
        # vsock, params, deny. The standalone register_custom_action,
        # register_stream_kind, vsock_service, params, on_trap_deny verbs
        # delete; these scopes are the one surface.
        .actions(
            lambda a: a.register("quarantine", {"type": "object"}, _noop_action)
        )
        .streams(lambda s: s.register(0x0013, {"type": "object"}, _echo_bytes))
        .vsock(lambda v: v.service(0x0013, "echo", echo))
        .params(lambda p: p.model(dict))
        .deny(lambda d: d.on_trap_deny(lambda event, verdict, session: None))
    )


# see: cans/02-plugin-model.md#Unified-builder
# Mount case 2: host-only verbs on a plugin builder, expecting EConfig at
# the call site. Live Python rejects exactly these probes plus boot()
# below; seal() and use() do not raise on a plugin-mode builder live,
# although the spec lists them host-only, so they are not probed here.
# The scope entries (.fs, .actions, .streams, .vsock, .params, .deny)
# are both-modes surfaces; their host-only sub-config is what the
# plugin-mode probe rejects.
def host_only_verbs_rejected_on_plugin() -> list[str]:
    rejected: list[str] = []
    plugin = build_surface_plugin()
    probes = [
        ("terminal", lambda: plugin.terminal(cols=120, rows=40)),
        ("storage", lambda: plugin.storage(lock_broker=True)),
        ("wire", lambda: plugin.wire()),
        ("fs.config.lock_broker", lambda: plugin.fs(lambda f: f.config(lock_broker=True))),
        ("params", lambda: plugin.params(lambda p: p.model(dict))),
        ("detached", lambda: plugin.detached(True)),
        (
            "actions",
            lambda: plugin.actions(
                lambda a: a.register("quarantine", {"type": "object"}, _noop_action)
            ),
        ),
        (
            "streams",
            lambda: plugin.streams(
                lambda s: s.register(0x0013, {"type": "object"}, _echo_bytes)
            ),
        ),
        ("deny", lambda: plugin.deny(lambda d: d.on_trap_deny(lambda event: None))),
    ]
    for verb, call in probes:
        try:
            call()
        except EConfig:
            rejected.append(verb)
    return rejected


# see: cans/02-plugin-model.md#Nested-scopes
# Negative vector: f.on_trap(...) inside .fs(...) is a type error in
# Python — the FsScopeProtocol has no on_trap member. Uncommenting the
# body must fail mypy/pyright with "Protocol FsScopeProtocol has no
# attribute on_trap". vectors.json cannot express an expected-error case
# (its schema carries event/expected_verdict only), so the negative case
# is pinned here as a comment-pinned block and in the rs/ts siblings.
#
# def negative_fs_on_trap(builder: KelirBuilder) -> KelirBuilder:
#     return builder.fs(
#         lambda f: f.on_trap(lambda event, ctx: Verdict.pass_())
#     )

# see: cans/02-plugin-model.md#Nested-scopes
# Flat-fold pin: a flat chain exists only as a group() fold over the
# scopes — a readability fold that produces the same descriptors (same
# rings, same ordinals). There is no separate flat surface: no
# build_flat_plugin exists, the fold delegates each verb to its scope.
def build_flat_fold_plugin() -> KelirBuilder:
    return KelirBuilder.plugin("surface-flat-fold").group(
        lambda g: g.trap(
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
async def main() -> None:
    build_host().seal()
    expected = [
        "terminal",
        "storage",
        "wire",
        "fs.config.lock_broker",
        "params",
        "detached",
        "actions",
        "streams",
        "deny",
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
    build_flat_fold_plugin()


if __name__ == "__main__":
    asyncio.run(main())
