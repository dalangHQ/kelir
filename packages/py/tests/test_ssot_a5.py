# see: cans/02-plugin-model.md#Unified-builder
# see: cans/03-sdk-architecture.md#Service-lookup-across-languages
# see: cans/11-protocol.md#Wire-messages
from __future__ import annotations

import asyncio
import inspect

import pytest

from kelir import (
    Approval,
    Caps,
    Deadline,
    FilterAction,
    HandlerContext,
    Handshake,
    KelirBuilder,
    Sandbox,
    Session,
    Verdict,
    VsockServiceEntry,
)
from kelir.service_caps import materialize_cap
from kelir.model_plugin import (
    PluginError,
    plugin_error_to_filter_action,
    plugin_error_to_verdict,
)
from kelir.plugins.plugin_ledger import LedgerPlugin
from kelir.sdk_errors import EConfig


async def _echo(request: bytes) -> bytes:
    return request


def test_plugin_class_receiver_creates_plugin_builder() -> None:
    plugin = KelirBuilder.plugin("chaos")
    assert plugin._mode == "plugin"
    assert plugin.name == "chaos"


def test_use_mounts_plugin_instance() -> None:
    host = KelirBuilder()
    assert host.use(LedgerPlugin()) is host
    assert [r.name for r in host._plugin_registrations] == ["ledger"]


def test_plugin_rejects_non_name_and_instance_plugin() -> None:
    with pytest.raises(EConfig, match=r"non-empty plugin name"):
        KelirBuilder.plugin(LedgerPlugin())
    with pytest.raises(EConfig, match=r"mount a plugin with \.use\(plugin\)"):
        KelirBuilder().plugin("ledger")
    with pytest.raises(EConfig, match=r"mount a plugin with \.use\(plugin\)"):
        KelirBuilder().plugin(LedgerPlugin())


def test_caps_have_typed_service_skeletons() -> None:
    cap = Caps()
    assert cap.policy.violations == 0
    assert cap.db_proxy.rewrite_target == ""
    assert cap.ledger is not None
    assert cap.transaction is not None
    assert cap.container is not None
    assert HandlerContext().cap.policy.violations == 0


def test_cap_skeleton_methods_raise_econfig_l3() -> None:
    cap = Caps()
    with pytest.raises(EConfig, match=r"L3"):
        asyncio.run(cap.ledger.flush())
    with pytest.raises(EConfig, match=r"L3"):
        asyncio.run(cap.ledger.blake3_of_producing_process("/bin/git"))
    with pytest.raises(EConfig, match=r"L3"):
        asyncio.run(cap.ledger.count_denials(100))
    with pytest.raises(EConfig, match=r"L3"):
        asyncio.run(cap.container.ingest_image("./fixtures/oci-hello-world.tar"))


def test_transaction_scope_shape() -> None:
    cap = Caps()
    scope = cap.transaction.scope(
        "migrate-42", paths=["/workspace/migrations"], schemas=["migrate_schema"]
    )
    assert scope.name == "migrate-42"
    assert scope.paths == ("/workspace/migrations",)
    assert scope.schemas == ("migrate_schema",)
    with pytest.raises(EConfig, match=r"L3"):
        asyncio.run(scope.savepoint())


def test_suspend_until_accepts_instances_only() -> None:
    assert Verdict.suspend(until=Approval()).payload["until"] is not None
    assert Verdict.suspend(until=Deadline(500)).payload["until"] == Deadline(500)
    assert Verdict.suspend(until=Handshake()).payload["until"] is not None
    assert Verdict.suspend().payload == {"until": None}
    assert FilterAction.suspend(until=Deadline(0)).payload["until"] == Deadline(0)
    with pytest.raises(EConfig, match=r"must be an instance"):
        Verdict.suspend(until=Approval)  # type: ignore[arg-type]
    with pytest.raises(EConfig, match=r"must be Approval"):
        Verdict.suspend(until="Approval")  # type: ignore[arg-type]


def test_session_and_sandbox_suspend_take_reason() -> None:
    assert "reason" in inspect.signature(Session.suspend).parameters
    assert "reason" in inspect.signature(Sandbox.suspend).parameters
    assert "path" not in inspect.signature(Session.suspend).parameters
    assert "path" not in inspect.signature(Session.resume).parameters


def test_vsock_service_takes_exactly_one_handler() -> None:
    builder = KelirBuilder()
    entry = VsockServiceEntry(port=0x0013, service="echo", schema={})
    assert builder.vsock_service(entry, _echo) is builder
    entry_only = VsockServiceEntry(
        port=0x0014, service="echo", schema={}, handler=_echo
    )
    assert builder.vsock_service(entry_only) is builder
    with pytest.raises(EConfig, match=r"passed twice"):
        both = VsockServiceEntry(
            port=0x0015, service="echo", schema={}, handler=_echo
        )
        builder.vsock_service(both, _echo)
    with pytest.raises(EConfig, match=r"declares no handler"):
        none = VsockServiceEntry(port=0x0016, service="echo", schema={})
        builder.vsock_service(none)


def test_materialize_cap_aliases_db_proxy_and_keeps_methods() -> None:
    cap = Caps()
    materialize_cap(cap, "db-proxy", {"rewrite_target": "migrate_schema."})
    assert cap.db_proxy.rewrite_target == "migrate_schema."
    materialize_cap(cap, "ledger", {"flush": "shadow", "store": "sqlite://x"})
    assert callable(cap.ledger.flush)
    assert cap.ledger.store == "sqlite://x"  # type: ignore[attr-defined]
    materialize_cap(cap, "chaos", {"drop_rate": 0.02})
    assert cap.chaos.drop_rate == 0.02  # type: ignore[attr-defined]


def test_plugin_error_maps_to_drop() -> None:
    err = PluginError.reject(errno="E_PATH_TRAVERSAL", reason="escapes root")
    verdict = plugin_error_to_verdict(err)
    assert verdict.kind == "drop"
    assert verdict.payload == {
        "errno": "E_PATH_TRAVERSAL",
        "reason": "escapes root",
    }
    action = plugin_error_to_filter_action(err)
    assert action.kind == "drop"
    assert action.payload == {
        "errno": "E_PATH_TRAVERSAL",
        "reason": "escapes root",
    }


def test_handler_context_carries_ctx_and_params() -> None:
    ctx = HandlerContext(ctx={"n": 1}, params="migrate")
    assert ctx.ctx == {"n": 1}
    assert ctx.params == "migrate"
