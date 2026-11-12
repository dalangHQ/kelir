# see: cans/03-sdk-architecture.md#Python-SDK
# see: cans/08-lifecycle.md#Detached-lifecycle
from __future__ import annotations

import asyncio
import importlib.util
import json
from datetime import timedelta
from pathlib import Path
from types import SimpleNamespace
from typing import Any, cast

import pytest

import kelir
from kelir import (
    Approval,
    Caps,
    ClockHandle,
    Deadline,
    ExecveMutation,
    FilterAction,
    HandlerContext,
    Handshake,
    KelirBuilder,
    PostExecAction,
    Sandbox,
    Session,
    TrapEvent,
    TrapFilter,
    Verdict,
    VsockServiceEntry,
    relay_socket_path,
)
from kelir.sdk_errors import EConfig
from kelir.plugin_fusion import FusedTrampoline, HookStage, RingEvent, TrapFilterSnapshot
from kelir.plugins import LedgerPlugin
from kelir.storage_planes import MaskRule, SyntheticFile


def test_deadline_defaults_to_zero() -> None:
    assert Deadline().value == 0
    assert Deadline(500).value == 500


def test_verdict_pass_is_empty() -> None:
    verdict = Verdict.pass_()
    assert verdict.kind == "pass"
    assert verdict.payload == {}
    assert Verdict.Pass().kind == "pass"
    assert Verdict.Pass().payload == {}


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
    mutation = verdict.payload["mutation"]
    assert isinstance(mutation, ExecveMutation)
    assert mutation.to_wire() == {
        "new_binary": "/bin/policy-allow",
        "new_argv": ["policy-allow", "--rewritten"],
        "set_env": [["KELIR_POLICY_REWRITE", "1"]],
        "unset_env": [],
    }


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


def test_trap_filter_pattern_snapshot() -> None:
    snapshot = TrapFilter.pattern("policy-*").snapshot()
    assert snapshot == TrapFilterSnapshot(pattern="policy-*")
    combined = TrapFilter.pattern("git-*").with_arg("--status").snapshot()
    assert combined == TrapFilterSnapshot(args=("--status",), pattern="git-*")


def test_builder_trap_trapfilter_pattern_snapshot() -> None:
    async def pre(ctx: object) -> None:
        return None

    builder = KelirBuilder()
    builder.trap(TrapFilter.pattern("policy-*"), pre=pre)
    chain = builder.fuse().chains[0]
    assert chain.hooks[0].filter == TrapFilterSnapshot(pattern="policy-*")


def test_session_surface_shapes() -> None:
    session = Session()
    assert session.driver_name == ""
    assert session.boot_ms == 0
    assert isinstance(session.cap, Caps)
    assert session.cap.db_proxy.rewrite_target == ""
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
    if not _native_bridge_built():
        pytest.skip("native _kelir extension is not built; run just build-py")
    with pytest.raises(EConfig, match=r"not 32 hex chars"):
        asyncio.run(Session.attach("nothex"))
    with pytest.raises(EConfig, match=r"Session\.attach \(L3\)"):
        asyncio.run(Session.attach("0123456789abcdef0123456789abcdef"))


# see: cans/08-lifecycle.md#Clock-skew
def test_clock_skew_timedelta_days_equals_86400_seconds() -> None:
    calls: list[float] = []

    class _StubVm:
        def clock_skew(self, seconds: float) -> None:
            calls.append(seconds)

    handle = ClockHandle(_StubVm())
    asyncio.run(handle.skew(timedelta(days=1)))
    assert calls == [86400.0]
    with pytest.raises(EConfig, match=r"timedelta"):
        asyncio.run(handle.skew(cast(Any, 86400.0)))
    with pytest.raises(EConfig, match=r"timedelta"):
        asyncio.run(handle.skew(cast(Any, 86_400_000)))


def _native_bridge_built() -> bool:
    return (
        importlib.util.find_spec("kelir._kelir") is not None
        or importlib.util.find_spec("_kelir") is not None
    )


def test_relay_socket_path_delegates_to_the_native_bridge() -> None:
    if not _native_bridge_built():
        pytest.skip("native _kelir extension is not built; run just build-py")
    canonical = "ABCDEF0123456789ABCDEF0123456789"
    expected = str(
        Path.home() / ".kelir" / "sessions" / canonical.lower() / "relay.sock"
    )
    assert relay_socket_path(canonical) == expected


def test_relay_socket_path_rejects_non_hex_ids() -> None:
    if not _native_bridge_built():
        pytest.skip("native _kelir extension is not built; run just build-py")
    with pytest.raises(EConfig, match=r"not 32 hex chars"):
        relay_socket_path("z" * 32)
    with pytest.raises(EConfig, match=r"not 32 hex chars"):
        relay_socket_path("abc")


def test_relay_socket_path_fails_closed_without_the_native_bridge(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def _missing_bridge() -> Any:
        raise EConfig("kelir native PyO3 extension (_kelir) is missing: install the precompiled wheel (L3)")

    monkeypatch.setattr(kelir, "_load_native_bridge", _missing_bridge)
    with pytest.raises(EConfig, match=r"_kelir\) is missing"):
        relay_socket_path("f" * 32)


def test_load_native_bridge_raises_econfig_when_import_fails(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    real_import: Any = importlib.import_module

    def _no_bridge(name: str) -> Any:
        if name in ("kelir._kelir", "_kelir"):
            raise ImportError(f"No module named {name!r}")
        return real_import(name)

    monkeypatch.setattr(importlib, "import_module", _no_bridge)
    with pytest.raises(EConfig, match=r"_kelir\) is missing"):
        kelir._load_native_bridge()


def test_load_native_bridge_raises_econfig_when_module_absent() -> None:
    if _native_bridge_built():
        pytest.skip("native _kelir extension is present; the missing path cannot fire")
    with pytest.raises(EConfig, match=r"_kelir\) is missing"):
        kelir._load_native_bridge()


def test_handler_context_shapes() -> None:
    ctx: HandlerContext[None, None] = HandlerContext()
    assert ctx.trap == TrapEvent()
    assert ctx.trap.pid == 0
    assert ctx.ctx is None
    with pytest.raises(EConfig, match=r"trap receipt"):
        asyncio.run(ctx.receipt())


def test_handler_context_verdict_convenience_constructs_canonical_verdicts() -> None:
    ctx: HandlerContext[None, None] = HandlerContext()
    passed = ctx.pass_()
    assert (passed.kind, passed.payload) == (Verdict.pass_().kind, Verdict.pass_().payload)
    dropped = ctx.drop("EPERM", "denied here")
    canonical_drop = Verdict.drop("EPERM", "denied here")
    assert (dropped.kind, dropped.payload) == (canonical_drop.kind, canonical_drop.payload)
    patched = ctx.patch(
        binary="/bin/policy-allow",
        argv=["policy-allow", "--rewritten"],
        env={"KELIR_POLICY_REWRITE": "1"},
    )
    canonical_patch = Verdict.patch(
        binary="/bin/policy-allow",
        argv=["policy-allow", "--rewritten"],
        env={"KELIR_POLICY_REWRITE": "1"},
    )
    assert patched.kind == canonical_patch.kind
    assert patched.payload["binary"] == canonical_patch.payload["binary"]
    assert patched.payload["argv"] == canonical_patch.payload["argv"]
    assert patched.payload["env"] == canonical_patch.payload["env"]
    assert patched.payload["mutation"].to_wire() == canonical_patch.payload["mutation"].to_wire()
    suspended = ctx.suspend(Deadline(500))
    canonical_suspend = Verdict.suspend(until=Deadline(500))
    assert (suspended.kind, suspended.payload) == (
        canonical_suspend.kind,
        canonical_suspend.payload,
    )
    aborted = ctx.abort("EPOLICY: abort row")
    canonical_abort = Verdict.abort("EPOLICY: abort row")
    assert (aborted.kind, aborted.payload) == (canonical_abort.kind, canonical_abort.payload)


# see: cans/02-plugin-model.md#Unified-verdict
def _canonical_verdict_wire(verdict: Verdict) -> Any:
    if verdict.kind == "pass":
        return "Pass"
    if verdict.kind == "drop":
        return {
            "Drop": {
                "errno": verdict.payload["errno"],
                "reason": verdict.payload["reason"],
            }
        }
    if verdict.kind == "patch":
        return {"Patch": {"ExecveMutation": verdict.payload["mutation"].to_wire()}}
    if verdict.kind == "suspend":
        until = verdict.payload["until"]
        if isinstance(until, Deadline):
            return {"Suspend": {"until": {"Deadline": until.value}}}
        if isinstance(until, Approval):
            return {"Suspend": {"until": "Approval"}}
        if isinstance(until, Handshake):
            return {"Suspend": {"until": "Handshake"}}
        return {"Suspend": {"until": until}}
    return {"Abort": {"reason": verdict.payload["reason"]}}


def test_handler_context_verdict_convenience_matches_vectors_wire_literals() -> None:
    ctx: HandlerContext[None, None] = HandlerContext()
    built = {
        "trap-git-commit-pass": _canonical_verdict_wire(ctx.pass_()),
        "trap-git-push-deny-epolicy": _canonical_verdict_wire(
            ctx.drop("EPERM", "EPOLICY: force push denied")
        ),
        "trap-pipe-install-rewrite-execve-mutation": _canonical_verdict_wire(
            ctx.patch(
                binary="/usr/local/bin/inspected-curl",
                argv=["inspected-curl", "-fsS"],
                env={"KELIR_TAINT": "1"},
                unset_env=["KELIR_UNTRUSTED"],
            )
        ),
        "trap-mutating-tool-suspend-approval": _canonical_verdict_wire(
            ctx.suspend(Approval())
        ),
        "trap-clock-skew-deadline-suspend": _canonical_verdict_wire(
            ctx.suspend(Deadline(60000))
        ),
        "trap-unverified-caller-abort-ewxviolation": _canonical_verdict_wire(
            ctx.abort("EWXVIOLATION: unverified caller binary")
        ),
    }
    vectors_path = Path(__file__).resolve().parents[3] / "vectors.json"
    vectors = json.loads(vectors_path.read_text(encoding="utf-8"))["vectors"]
    matched = 0
    for vector in vectors:
        name = vector["name"]
        if name not in built:
            continue
        matched += 1
        assert built[name] == vector["expected_verdict"], (
            f"HandlerContext convenience verdict diverges from the {name} wire literal"
        )
    assert matched == len(built), (
        f"vectors.json lacks the convenience-verdict vectors: matched {matched}"
    )


def test_boot_requires_native_plugin_contexts_for_caps(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class _StubHandle:
        driver_name = "auto"
        boot_ms = 0.0
        vm = None
        terminal = None
        storage = None
        wire = None
        checkpoint = None
        host = None

    class _StubNativeBuilder:
        def __init__(self, **kwargs: Any) -> None:
            return None

        def use_plugin(self, name: str, enabled: bool) -> None:
            return None

        def fuse_hooks(self, descriptor: dict[str, Any]) -> None:
            return None

        def boot(self) -> _StubHandle:
            return _StubHandle()

    monkeypatch.setattr(
        kelir,
        "_load_native_bridge",
        lambda: SimpleNamespace(PyKelirBuilder=_StubNativeBuilder),
    )
    builder = KelirBuilder().use(LedgerPlugin())
    with pytest.raises(EConfig, match=r"get_plugin_context"):
        asyncio.run(builder.boot())


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


def test_builder_vsock_service_validates_u32_port() -> None:
    async def handler(request: bytes) -> bytes:
        return request

    builder = KelirBuilder()
    entry = VsockServiceEntry(port=0, service="echo", schema={})
    assert builder.vsock_service(entry, handler) is builder
    entry = VsockServiceEntry(port=0xFFFFFFFF, service="echo", schema={})
    assert builder.vsock_service(entry, handler) is builder
    assert len(builder.vsock_services) == 2
    assert builder.vsock_services[0].port == 0
    assert builder.vsock_services[1].port == 0xFFFFFFFF
    with pytest.raises(EConfig, match=r"outside u32"):
        entry = VsockServiceEntry(port=-1, service="echo", schema={})
        builder.vsock_service(entry, handler)
    with pytest.raises(EConfig, match=r"outside u32"):
        entry = VsockServiceEntry(port=0x100000000, service="echo", schema={})
        builder.vsock_service(entry, handler)
    with pytest.raises(EConfig, match=r"outside u32"):
        entry = VsockServiceEntry(port=True, service="echo", schema={})
        builder.vsock_service(entry, handler)


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


def test_builder_boot_requires_native_bridge(monkeypatch: pytest.MonkeyPatch) -> None:
    def _missing_bridge() -> Any:
        raise EConfig("kelir native PyO3 extension (_kelir) is missing: install the precompiled wheel (L3)")

    monkeypatch.setattr(kelir, "_load_native_bridge", _missing_bridge)
    with pytest.raises(EConfig, match=r"_kelir\) is missing"):
        asyncio.run(KelirBuilder().boot())


def test_builder_boot_with_native_bridge(tmp_path: Path) -> None:
    if (
        importlib.util.find_spec("kelir._kelir") is None
        and importlib.util.find_spec("_kelir") is None
    ):
        pytest.skip("native _kelir extension is not built; run just build-py")
    kernel = tmp_path / "vmlinux"
    rootfs = tmp_path / "rootfs.erofs"
    kernel.write_bytes(b"")
    rootfs.write_bytes(b"")
    session = asyncio.run(
        KelirBuilder(kernel=str(kernel), rootfs_erofs=str(rootfs)).boot()
    )
    assert session.driver_name == "auto"
    assert session.boot_ms >= 0
    child = asyncio.run(session.fork())
    assert child.driver_name == "auto"
    stats = asyncio.run(session.storage.reconcile())
    assert stats.files_changed == 0
    assert stats.tombstones_resolved == 0
    asyncio.run(child.stop())
    asyncio.run(session.stop())


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


# see: cans/plugins/profile.md#Bidirectional-extraction
def test_sandbox_export_spec_round_trips_every_fixture_section(tmp_path: Path) -> None:
    yaml_module = pytest.importorskip("yaml")
    fixture = (
        Path(__file__).resolve().parents[3]
        / "cans"
        / "artifacts"
        / "examples"
        / "fixtures"
        / "kelir.yaml"
    )
    loaded = yaml_module.safe_load(fixture.read_text(encoding="utf-8"))
    sandbox = Sandbox.from_profile(loaded, str(fixture))
    out = tmp_path / "export.yaml"
    asyncio.run(sandbox.export_spec(str(out)))
    exported = yaml_module.safe_load(out.read_text(encoding="utf-8"))
    assert list(exported.keys()) == list(loaded.keys())
    assert exported == loaded


# see: cans/plugins/profile.md#Inversion-of-Control-configuration-router
def test_sandbox_from_profile_rejects_unknown_root_sections() -> None:
    profile = {"session": {}, "mystery_section": {"key": "value"}}
    with pytest.raises(EConfig, match=r"invalid profile schema"):
        Sandbox.from_profile(profile, "profile.toml")


def test_sandbox_before_boot_raises_econfig() -> None:
    sandbox = Sandbox.from_profile({}, "profile.toml")
    with pytest.raises(EConfig, match=r"Sandbox\.exec before boot"):
        asyncio.run(sandbox.exec("/bin/ls"))
    with pytest.raises(EConfig, match=r"Sandbox\.rollback before boot"):
        asyncio.run(sandbox.rollback())
    with pytest.raises(EConfig, match=r"Sandbox\.suspend before boot"):
        asyncio.run(sandbox.suspend(kelir.ResourcePressure()))
    with pytest.raises(EConfig, match=r"Sandbox\.resume before boot"):
        asyncio.run(sandbox.resume())
    with pytest.raises(EConfig, match=r"Sandbox\.reconcile before boot"):
        asyncio.run(sandbox.reconcile())
    with pytest.raises(EConfig, match=r"Sandbox\.stop before boot"):
        asyncio.run(sandbox.stop())
    with pytest.raises(EConfig, match=r"Sandbox\.terminate before boot"):
        asyncio.run(sandbox.terminate())


def test_sandbox_terminate_routes_to_abrupt_session_verb() -> None:
    sandbox = Sandbox.from_profile({}, "profile.toml")
    calls: list[str] = []

    class _StubSession:
        async def stop(self) -> None:
            calls.append("stop")

        async def terminate(self) -> None:
            calls.append("terminate")

    sandbox._session = cast(Any, _StubSession())
    asyncio.run(sandbox.terminate())
    assert calls == ["terminate"]


def test_sandbox_boot_requires_native_bridge(monkeypatch: pytest.MonkeyPatch) -> None:
    def _missing_bridge() -> Any:
        raise EConfig("kelir native PyO3 extension (_kelir) is missing: install the precompiled wheel (L3)")

    monkeypatch.setattr(kelir, "_load_native_bridge", _missing_bridge)
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
