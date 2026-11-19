# see: cans/02-plugin-model.md#Segregated-ring-traits
# see: cans/05-storage-plane.md#Interception-pipeline
# see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
# see: cans/10-attestation.md#Execution-receipts
from __future__ import annotations

import asyncio
from pathlib import Path

import pytest

import kelir
from kelir import (
    Errno,
    ExecutionReceipt,
    HandlerContext,
    KelirBuilder,
    PatchPayload,
    Session,
    SpawnResult,
    TrapEvent,
    Verdict,
)
from kelir.generated.attest_telemetry import ExitKindNormal, ExitKindSignal
from kelir.plugin_fusion import RingEvent
from kelir.plugin_model import BasePlugin, hook_descriptors
from kelir.sdk_errors import EConfig
from kelir.wire_planes import WireConfig


class QuarantinePlugin(BasePlugin[None, None]):
    name = "quarantine"

    def on_fs_unlink(self, path: Path, ctx: HandlerContext[None, None]) -> Verdict:
        return Verdict.patch(
            PatchPayload.fs_path_redirect(
                target_path=f"/workspace/.agentsh_trash/{path.name}"
            )
        )

    def on_fs_rename(
        self, src: Path, dst: Path, ctx: HandlerContext[None, None]
    ) -> Verdict:
        return Verdict.drop(errno=Errno.PATH_TRAVERSAL, reason="rename escapes root")


def test_errno_eafnosupport_value() -> None:
    assert Errno.EAFNOSUPPORT == "EAFNOSUPPORT"
    assert Errno("EAFNOSUPPORT") is Errno.EAFNOSUPPORT


def test_verdict_drop_accepts_eafnosupport() -> None:
    verdict = Verdict.drop(errno=Errno.EAFNOSUPPORT, reason="AF_RXRPC blocked")
    assert verdict.kind == "drop"
    assert verdict.payload["errno"] == "EAFNOSUPPORT"


def test_patch_payload_fs_path_redirect() -> None:
    payload = PatchPayload.fs_path_redirect(target_path="/workspace/.agentsh_trash/x")
    assert payload.kind == "fs-path-redirect"
    assert payload.target_path == "/workspace/.agentsh_trash/x"
    assert payload.to_wire() == {
        "kind": "fs-path-redirect",
        "target_path": "/workspace/.agentsh_trash/x",
    }


def test_verdict_patch_carries_fs_path_redirect() -> None:
    payload = PatchPayload.fs_path_redirect(target_path="/workspace/.agentsh_trash/x")
    verdict = Verdict.patch(payload)
    assert verdict.kind == "patch"
    assert verdict.payload["attr"] is payload


def test_trap_event_tree_depth_defaults_zero() -> None:
    assert TrapEvent().tree_depth == 0
    assert TrapEvent(tree_depth=17).tree_depth == 17


def test_base_plugin_fs_unlink_rename_default_pass() -> None:
    plugin = BasePlugin[None, None]()
    ctx: HandlerContext[None, None] = HandlerContext()
    assert plugin.on_fs_unlink(Path("/workspace/x"), ctx).kind == "pass"
    assert plugin.on_fs_rename(Path("/a"), Path("/b"), ctx).kind == "pass"


def test_base_plugin_fs_unlink_rename_override_registers_rings() -> None:
    descriptors = hook_descriptors(QuarantinePlugin())
    assert [d.ring for d in descriptors] == [RingEvent.FS_UNLINK, RingEvent.FS_RENAME]
    builder = QuarantinePlugin().to_builder()
    assert [d.ring for d in builder.hook_descriptors()] == [
        RingEvent.FS_UNLINK,
        RingEvent.FS_RENAME,
    ]


def test_builder_fs_unlink_rename_verbs_and_scope() -> None:
    plugin = KelirBuilder.plugin("fs-verbs").fs(
        lambda f: f.on_fs_unlink(lambda _p, _c: Verdict.pass_()).on_fs_rename(
            lambda _s, _d, _c: Verdict.pass_()
        )
    )
    assert [d.ring for d in plugin.hook_descriptors()] == [
        RingEvent.FS_UNLINK,
        RingEvent.FS_RENAME,
    ]
    host = kelir.builder()
    host.fs(
        lambda scope: scope.on_fs_unlink(lambda _p, _c: Verdict.pass_()).on_fs_rename(
            lambda _s, _d, _c: Verdict.pass_()
        )
    )
    assert [d.ring for d in host.hook_descriptors()] == [
        RingEvent.FS_UNLINK,
        RingEvent.FS_RENAME,
    ]


def test_builder_fs_unlink_arity_checked() -> None:
    with pytest.raises(EConfig, match=r"on_fs_unlink.*takes 1 positional"):
        KelirBuilder.plugin("arity").fs(
            lambda f: f.on_fs_unlink(lambda _p: Verdict.pass_())
        )
    with pytest.raises(EConfig, match=r"on_fs_rename.*takes 2 positional"):
        KelirBuilder.plugin("arity").fs(
            lambda f: f.on_fs_rename(lambda _s, _d: Verdict.pass_())
        )


def test_wire_config_divert_port_to_unix() -> None:
    builder = kelir.builder()
    wire = WireConfig()
    wire.divert_port_to_unix(5433, "/run/agentsh/pgwire.sock")
    wire.divert_port_to_unix(8000, "/run/agentsh/mcp.sock")
    builder.wire(wire)
    assert builder._config["wire"] == {
        "dial_timeout_ms": 5000,
        "divert_ports": [
            {"listen_port": 5433, "forward_unix_socket": "/run/agentsh/pgwire.sock"},
            {"listen_port": 8000, "forward_unix_socket": "/run/agentsh/mcp.sock"},
        ],
    }
    bridged = kelir._bridge_safe_config("wire", builder._config["wire"])
    assert bridged["divert_ports"] == builder._config["wire"]["divert_ports"]


def test_wire_config_closure_sets_netstack_and_egress() -> None:
    builder = kelir.builder()
    wire = WireConfig()
    wire.userspace_netstack = True
    wire.default_egress = "drop"
    builder.wire(wire)
    assert builder._config["wire"] == {
        "userspace_netstack": True,
        "default_egress": "drop",
        "dial_timeout_ms": 5000,
    }


def test_wire_config_divert_port_validates() -> None:
    config = WireConfig()
    with pytest.raises(EConfig, match=r"outside 1\.\.65535"):
        config.divert_port_to_unix(0, "/run/x.sock")
    with pytest.raises(EConfig, match=r"non-empty unix socket path"):
        config.divert_port_to_unix(5433, "")


def test_wire_config_is_host_only() -> None:
    plugin_untyped: KelirBuilder = KelirBuilder.plugin("p")
    with pytest.raises(EConfig, match=r"host-only verb \.wire\(\)"):
        plugin_untyped.wire(WireConfig())


def test_terminal_config_bridge_accepts_hierarchy_only() -> None:
    builder = kelir.builder()
    builder.terminal(
        cols=120,
        rows=40,
        wireframe_grid={"track_screen_matrix": True, "export_on_trap": False},
        sanitizer={"drop_osc52": True, "strip_title": True},
        stream_engine="StreamingAutomata",
    )
    bridged = kelir._bridge_safe_config("terminal", builder._config["terminal"])
    assert bridged["wireframe_grid"] == {"track_screen_matrix": True, "export_on_trap": False}
    assert bridged["sanitizer"] == {"drop_osc52": True, "strip_title": True}
    with pytest.raises(EConfig, match=r"unknown terminal plane config keys: drop_osc52"):
        kelir._bridge_safe_config("terminal", {"drop_osc52": True})


def test_storage_config_bridge_structures_shared_caches() -> None:
    builder = kelir.builder()
    builder.storage(
        shared_caches=[{"host_path": "/host/cache", "guest_mount": "/guest/cache", "read_only": True}],
        dax_window="off",
    )
    bridged = kelir._bridge_safe_config("storage", builder._config["storage"])
    assert bridged["shared_caches"] == [
        {"host_path": "/host/cache", "guest_mount": "/guest/cache", "read_only": True}
    ]
    assert bridged["dax_window"] == "off"


def test_builder_embed_mode_kwarg() -> None:
    assert kelir.builder(embed_mode="in_process_libkelir")._config["embed_mode"] == (
        "in_process_libkelir"
    )
    assert kelir.builder()._config["embed_mode"] is None


def test_session_elevate_lease_and_handler_lease_check() -> None:
    session = Session()
    assert HandlerContext(session=session).has_valid_operator_lease() is False
    assert HandlerContext().has_valid_operator_lease() is False
    asyncio.run(session.elevate_lease("signed_jws_operator_lease_token_payload"))
    assert HandlerContext(session=session).has_valid_operator_lease() is True


def test_session_elevate_lease_rejects_empty_token() -> None:
    with pytest.raises(EConfig, match=r"non-empty lease token"):
        asyncio.run(Session().elevate_lease(""))


def test_spawn_result_receipt_defaults_none_and_exit_kind_normal() -> None:
    result = SpawnResult()
    assert result.receipt is None
    assert result.exit_kind == ExitKindNormal()
    receipt = ExecutionReceipt(pid=4242, exit_kind=ExitKindSignal(signal=9))
    signalled = SpawnResult(
        exit_code=137, receipt=receipt, exit_kind=ExitKindSignal(signal=9)
    )
    assert signalled.receipt is receipt
    assert isinstance(signalled.exit_kind, ExitKindSignal)
    assert signalled.exit_kind.signal == 9
