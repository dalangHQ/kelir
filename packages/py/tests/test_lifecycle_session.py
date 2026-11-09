# see: cans/03-sdk-architecture.md#Python-SDK
# see: cans/08-lifecycle.md#Detached-lifecycle
from __future__ import annotations

import asyncio
from pathlib import Path

import pytest

from kelir import (
    Caps,
    Deadline,
    FilterAction,
    HandlerContext,
    KelirBuilder,
    PostExecAction,
    Sandbox,
    Session,
    TrapEvent,
    TrapFilter,
    Verdict,
    relay_socket_path,
)
from kelir.sdk_errors import EConfig
from kelir.plugin_fusion import FusedTrampoline, HookStage, RingEvent, TrapFilterSnapshot
from kelir.storage_planes import MaskRule, SyntheticFile


def test_deadline_defaults_to_zero() -> None:
    assert Deadline().value == 0
    assert Deadline(500).value == 500


def test_verdict_pass_is_empty() -> None:
    verdict = Verdict.pass_()
    assert verdict.kind == "pass"
    assert verdict.payload == {}


def test_verdict_drop_carries_errno_and_reason() -> None:
    default = Verdict.drop()
    assert default.kind == "drop"
    assert default.payload == {"errno": "", "reason": ""}
    denied = Verdict.drop("EPOLICY", "policy denies this binary")
    assert denied.payload == {"errno": "EPOLICY", "reason": "policy denies this binary"}


def test_verdict_patch_carries_mutation_payload() -> None:
    verdict = Verdict.patch(
        binary="/bin/policy-allow",
        argv=["policy-allow", "--rewritten"],
        env={"KELIR_POLICY_REWRITE": "1"},
    )
    assert verdict.kind == "patch"
    assert verdict.payload["binary"] == "/bin/policy-allow"
    assert verdict.payload["argv"] == ["policy-allow", "--rewritten"]
    assert verdict.payload["env"] == {"KELIR_POLICY_REWRITE": "1"}
    assert verdict.payload["name"] is None
    assert verdict.payload["cbor"] is None
    assert verdict.payload["attr"] is None


def test_verdict_patch_positional_arg_binds_attr() -> None:
    verdict = Verdict.patch(0o755)
    assert verdict.payload["attr"] == 0o755


def test_verdict_suspend_defaults_until_none() -> None:
    default = Verdict.suspend()
    assert default.kind == "suspend"
    assert default.payload == {"until": None}
    until = Verdict.suspend(until=Deadline(500))
    assert until.payload["until"] == Deadline(500)


def test_verdict_abort_defaults_empty_reason() -> None:
    default = Verdict.abort()
    assert default.kind == "abort"
    assert default.payload == {"reason": ""}
    assert Verdict.abort("EPOLICY: abort row").payload == {"reason": "EPOLICY: abort row"}


def test_post_exec_action_kinds() -> None:
    assert PostExecAction.proceed().kind == "proceed"
    assert PostExecAction.rollback_and_halt().kind == "rollback_and_halt"


def test_filter_action_lattice() -> None:
    assert FilterAction.forward().kind == "forward"
    assert FilterAction.forward().payload == {}
    assert FilterAction.drop("EPERM", "nope").payload == {"errno": "EPERM", "reason": "nope"}
    assert FilterAction.suspend().payload == {"until": None}
    assert FilterAction.abort("stop").payload == {"reason": "stop"}


def test_trap_event_defaults() -> None:
    event = TrapEvent()
    assert event.binary_path == ""
    assert event.argv == []
    assert event.pid == 0
    assert event.ppid == 0
    assert event.cwd == ""
    assert event.binary_blake3 == ""


def test_trap_filter_binary_and_args_snapshot() -> None:
    snapshot = TrapFilter.binary("git").with_arg("--status").snapshot()
    assert snapshot == TrapFilterSnapshot(binary="git", args=("--status",))
    assert snapshot.binary == "git"
    assert snapshot.args == ("--status",)
    assert snapshot.pattern is None


def test_trap_filter_empty_snapshot() -> None:
    snapshot = TrapFilter().snapshot()
    assert snapshot.binary is None
    assert snapshot.args == ()
    assert snapshot.pattern is None


def test_session_surface_shapes() -> None:
    session = Session()
    assert session.driver_name == ""
    assert session.boot_ms == 0
    assert isinstance(session.cap, Caps)
    assert session.cap.db.rewrite_target == "migrate_schema."
    assert session.cap.policy.violations == 0


def test_session_native_calls_raise_econfig() -> None:
    session = Session()
    with pytest.raises(EConfig, match=r"session spawn"):
        asyncio.run(session.spawn("/bin/ls"))
    with pytest.raises(EConfig, match=r"session stop"):
        asyncio.run(session.stop())
    with pytest.raises(EConfig, match=r"session rollback"):
        asyncio.run(session.rollback())
    with pytest.raises(EConfig, match=r"session console"):
        asyncio.run(session.console())


def test_session_plane_handles_raise_econfig_without_native_bridge() -> None:
    session = Session()
    with pytest.raises(EConfig, match=r"vm checkpoint"):
        asyncio.run(session.vm.checkpoint("snap"))
    with pytest.raises(EConfig, match=r"storage reconcile"):
        asyncio.run(session.storage.reconcile())
    with pytest.raises(EConfig, match=r"wire forward_port"):
        asyncio.run(session.wire.forward_port(8080, 18080))
    with pytest.raises(EConfig, match=r"terminal notice"):
        session.terminal.notice("hello")
    with pytest.raises(EConfig, match=r"vsock channel"):
        asyncio.run(session.vsock.channel(0x0013))
    with pytest.raises(EConfig, match=r"host lsm_status"):
        asyncio.run(session.host.lsm_status())


def test_session_attach_requires_l3_bridge() -> None:
    with pytest.raises(EConfig, match=r"not 32 hex chars"):
        asyncio.run(Session.attach("nothex"))
    with pytest.raises(EConfig, match=r"Session\.attach \(L3\)"):
        asyncio.run(Session.attach("0123456789abcdef0123456789abcdef"))


def test_relay_socket_path_lowercases_hex_session_id() -> None:
    session_id = "0123456789ABCDEF0123456789ABCDEF"
    expected = str(Path.home() / ".kelir" / "sessions" / session_id.lower() / "relay.sock")
    assert relay_socket_path(session_id) == expected


def test_relay_socket_path_rejects_non_hex_ids() -> None:
    with pytest.raises(EConfig, match=r"not 32 hex chars"):
        relay_socket_path("z" * 32)
    with pytest.raises(EConfig, match=r"not 32 hex chars"):
        relay_socket_path("abc")


def test_handler_context_shapes() -> None:
    ctx = HandlerContext()
    assert ctx.trap == TrapEvent()
    assert ctx.trap.pid == 0
    assert ctx.ctx is None
    with pytest.raises(EConfig, match=r"trap receipt"):
        asyncio.run(ctx.receipt())


def test_caps_services_raise_econfig_without_native_bridge() -> None:
    caps = Caps()
    with pytest.raises(EConfig, match=r"ledger flush"):
        asyncio.run(caps.ledger.flush())
    with pytest.raises(EConfig, match=r"ledger count_denials"):
        asyncio.run(caps.ledger.count_denials(1))
    with pytest.raises(EConfig, match=r"ledger finalize"):
        asyncio.run(caps.ledger.finalize())
    with pytest.raises(EConfig, match=r"transaction begin_scope"):
        asyncio.run(caps.transaction.begin_scope("blast-radius"))


def test_builder_defaults_and_detached_flag() -> None:
    assert KelirBuilder().is_detached() is False
    assert KelirBuilder().detached(True).is_detached() is True
    assert KelirBuilder(detached=True).is_detached() is True
    assert KelirBuilder().vsock_services == []


def test_builder_fuse_of_empty_hook_set() -> None:
    trampoline = KelirBuilder().fuse()
    assert isinstance(trampoline, FusedTrampoline)
    assert trampoline.chains == ()
    assert trampoline.crossings_per_ring_event(RingEvent.TRAP) == 0
    assert trampoline.hook_count(RingEvent.TRAP) == 0
    assert trampoline.describe() == {"rings": []}


def test_builder_trap_registers_pre_and_post_hooks() -> None:
    async def pre(ctx: object) -> None:
        return None

    async def post(ctx: object) -> None:
        return None

    builder = KelirBuilder()
    builder.trap(TrapFilter.binary("git"), pre=pre, post=post)
    trampoline = builder.fuse()
    assert trampoline.hook_count(RingEvent.TRAP) == 2
    chain = trampoline.chains[0]
    assert chain.ring is RingEvent.TRAP
    assert [hook.ordinal for hook in chain.hooks] == [0, 1]
    assert [hook.stage for hook in chain.hooks] == [HookStage.PRE, HookStage.POST]
    assert chain.hooks[0].filter == TrapFilterSnapshot(binary="git", args=())


def test_builder_trap_pattern_snapshot() -> None:
    async def pre(ctx: object) -> None:
        return None

    builder = KelirBuilder()
    builder.trap("policy-*", pre=pre)
    chain = builder.fuse().chains[0]
    assert chain.hooks[0].filter == TrapFilterSnapshot(pattern="policy-*")


def test_builder_lifecycle_decorator_registers_single_hook() -> None:
    builder = KelirBuilder()

    @builder.on_vm_ready
    async def on_ready(ctx: object) -> None:
        return None

    trampoline = builder.fuse()
    assert trampoline.hook_count(RingEvent.LIFECYCLE) == 1
    hook = trampoline.chains[0].hooks[0]
    assert hook.stage is HookStage.SINGLE
    assert hook.lifecycle_tag == "vm-ready"


def test_builder_register_vsock_service_validates_u32_port() -> None:
    async def handler(request: bytes) -> bytes:
        return request

    builder = KelirBuilder()
    assert builder.register_vsock_service(0, "echo", {}, handler) is builder
    assert builder.register_vsock_service(0xFFFFFFFF, "echo", {}, handler) is builder
    assert len(builder.vsock_services) == 2
    assert builder.vsock_services[0].port == 0
    assert builder.vsock_services[1].port == 0xFFFFFFFF
    with pytest.raises(EConfig, match=r"outside u32"):
        builder.register_vsock_service(-1, "echo", {}, handler)
    with pytest.raises(EConfig, match=r"outside u32"):
        builder.register_vsock_service(0x100000000, "echo", {}, handler)
    with pytest.raises(EConfig, match=r"outside u32"):
        builder.register_vsock_service(True, "echo", {}, handler)


def test_builder_register_stream_kind_validates_u16_at_fuse() -> None:
    async def handler(request: bytes) -> bytes:
        return request

    builder = KelirBuilder()
    builder.register_stream_kind(0x0013, {"type": "object"}, handler)
    assert builder.fuse().hook_count(RingEvent.VSOCK_SERVICE) == 1
    out_of_range = KelirBuilder()
    out_of_range.register_stream_kind(0x10000, {"type": "object"}, handler)
    with pytest.raises(EConfig, match=r"outside u16"):
        out_of_range.fuse()


def test_builder_boot_requires_native_bridge() -> None:
    with pytest.raises(EConfig, match=r"_kelir\) is missing"):
        asyncio.run(KelirBuilder().boot())


def test_sandbox_create_is_pure_builder_state() -> None:
    sandbox = asyncio.run(Sandbox.create(vcpus=2, memory_mb=2048))
    assert sandbox.session is None
    assert sandbox.session_id() is None
    assert sandbox.source_path == ""
    assert sandbox.profile == {}
    assert sandbox.is_detached() is False
    detached = asyncio.run(Sandbox.create(detached=True))
    assert detached.is_detached() is True


def test_sandbox_from_profile_exposes_session_id() -> None:
    profile = {"session": {"id": "sess-001", "compute": {"vcpus": 4, "memory_mb": 2048}}}
    sandbox = Sandbox.from_profile(profile, "profile.toml")
    assert sandbox.session_id() == "sess-001"
    assert sandbox.profile == profile
    assert sandbox.source_path == "profile.toml"
    assert Sandbox.from_profile({}, "profile.toml").session_id() is None


def test_sandbox_from_config_loads_toml(tmp_path: Path) -> None:
    profile = tmp_path / "profile.toml"
    profile.write_text(
        '[session]\nid = "sess-toml"\n\n[session.compute]\nvcpus = 2\nmemory_mb = 2048\n',
        encoding="utf-8",
    )
    sandbox = asyncio.run(Sandbox.from_config(str(profile)))
    assert sandbox.session_id() == "sess-toml"
    assert sandbox.source_path == str(profile)


def test_sandbox_from_config_rejects_unsupported_extension(tmp_path: Path) -> None:
    profile = tmp_path / "profile.json"
    profile.write_text("{}", encoding="utf-8")
    with pytest.raises(EConfig, match=r"unsupported profile extension"):
        asyncio.run(Sandbox.from_config(str(profile)))


def test_sandbox_export_spec_renders_known_toml_fields(tmp_path: Path) -> None:
    profile = {"session": {"id": "s1", "compute": {"vcpus": 4, "memory_mb": 2048}}}
    sandbox = Sandbox.from_profile(profile, "profile.toml")
    out = tmp_path / "export.toml"
    asyncio.run(sandbox.export_spec(str(out)))
    assert out.read_text(encoding="utf-8") == (
        '[session]\nid = "s1"\n\n[session.compute]\nvcpus = 4\nmemory_mb = 2048\n'
    )


def test_sandbox_before_boot_raises_econfig() -> None:
    sandbox = Sandbox.from_profile({}, "profile.toml")
    with pytest.raises(EConfig, match=r"Sandbox\.exec before boot"):
        asyncio.run(sandbox.exec("/bin/ls"))
    with pytest.raises(EConfig, match=r"Sandbox\.rollback before boot"):
        asyncio.run(sandbox.rollback())
    with pytest.raises(EConfig, match=r"Sandbox\.suspend before boot"):
        asyncio.run(sandbox.suspend("/tmp/snap"))
    with pytest.raises(EConfig, match=r"Sandbox\.resume before boot"):
        asyncio.run(sandbox.resume("/tmp/snap"))
    with pytest.raises(EConfig, match=r"Sandbox\.reconcile before boot"):
        asyncio.run(sandbox.reconcile())
    with pytest.raises(EConfig, match=r"Sandbox\.stop before boot"):
        asyncio.run(sandbox.stop())


def test_sandbox_boot_requires_native_bridge() -> None:
    sandbox = asyncio.run(Sandbox.create(vcpus=2))
    with pytest.raises(EConfig, match=r"_kelir\) is missing"):
        asyncio.run(sandbox.boot())


def test_storage_planes_defaults() -> None:
    assert MaskRule().glob == ""
    assert MaskRule().patterns == []
    static = SyntheticFile.static("/etc/motd", "hello")
    assert static.path == "/etc/motd"
    assert static.content == "hello"
    assert static.dynamic_fn is None
    assert SyntheticFile.dynamic("/proc/uptime", len).dynamic_fn is not None
