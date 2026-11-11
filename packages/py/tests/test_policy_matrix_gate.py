# see: cans/02-plugin-model.md#Foreign-plugin-proxy

from __future__ import annotations

import asyncio
import platform
from dataclasses import dataclass, field
from typing import Any, Awaitable, Callable

import pytest

import kelir
from kelir import Approval, Deadline, HandlerContext, KelirBuilder, TrapFilter, Verdict
from kelir.sdk_errors import EConfig
from kelir.plugin_fusion import HookStage, RingEvent
from kelir.model_plugin import (
    BasePlugin,
    VetoEvent,
    WatchdogEvent,
    WatchdogVerdict,
)


@dataclass
class PolicyState:
    ready: bool = False
    planned_suspend: bool = False
    mitigations: int = 0
    suppressions: int = 0
    quarantined_pids: list[int] = field(default_factory=list)


POLICY_STATE = PolicyState()


@pytest.fixture(autouse=True)
def _fresh_policy_state() -> None:
    POLICY_STATE.ready = False
    POLICY_STATE.planned_suspend = False
    POLICY_STATE.mitigations = 0
    POLICY_STATE.suppressions = 0
    POLICY_STATE.quarantined_pids = []


class PolicyMatrixPlugin(BasePlugin):
    name = "policy-matrix"

    async def on_vm_init(self, ctx: Any) -> None:
        ctx.register_veto(VetoEvent.Suspend, lambda: POLICY_STATE.ready)
        ctx.register_veto(VetoEvent.ClockSkew, lambda: POLICY_STATE.ready)

    async def on_vm_ready(self, _ctx: Any) -> None:
        POLICY_STATE.ready = True

    async def on_watchdog(self, event: WatchdogEvent) -> WatchdogVerdict:
        if event.last_active_pid is not None:
            POLICY_STATE.mitigations += 1
            return WatchdogVerdict.Mitigate
        return WatchdogVerdict.ExecuteDefault


class WatchdogSilencerPlugin(BasePlugin):

    name = "watchdog-silencer"

    async def on_watchdog(self, _event: WatchdogEvent) -> WatchdogVerdict:
        if POLICY_STATE.planned_suspend:
            POLICY_STATE.suppressions += 1
            return WatchdogVerdict.Suppress
        return WatchdogVerdict.ExecuteDefault


PreExecHandler = Callable[[HandlerContext], Awaitable[Verdict]]


async def row_allow(c: HandlerContext) -> Verdict:
    return Verdict.pass_()


async def row_deny(c: HandlerContext) -> Verdict:
    return Verdict.drop("EPOLICY", "EPOLICY: policy-matrix denies this binary.")


async def row_rewrite(c: HandlerContext) -> Verdict:
    return Verdict.patch(
        binary="/bin/policy-allow",
        argv=["policy-allow", "--rewritten"],
        env={"KELIR_POLICY_REWRITE": "1"},
    )


async def row_abort(c: HandlerContext) -> Verdict:
    return Verdict.abort("EPOLICY: abort row")


async def row_yield(c: HandlerContext) -> Verdict:
    await c.vm.checkpoint("policy-yield")
    return Verdict.suspend(until=Approval)


async def row_approval(c: HandlerContext) -> Verdict:
    return Verdict.suspend(until=Deadline(500))


async def row_custom(c: HandlerContext) -> Verdict:
    return Verdict.patch(
        name="quarantine",
        cbor={"pid": c.trap.pid, "reason": "policy-matrix quarantine row"},
    )


def build_runtime() -> KelirBuilder:
    builder = kelir.builder(
        vcpus=2,
        memory_mb=2048,
        kernel=f"./fixtures/vmlinux-6.6-{platform.machine()}-kelir",
        rootfs_erofs="./fixtures/rootfs-policy.erofs",
        tsc_locked=True,
        enable_watchdog=True,
    )

    builder = builder.plugin(PolicyMatrixPlugin())
    builder = builder.plugin(WatchdogSilencerPlugin())

    async def quarantine(payload: dict[str, Any]) -> None:
        POLICY_STATE.quarantined_pids.append(payload["pid"])

    builder.register_custom_action(
        "quarantine",
        {"type": "object",
         "properties": {"pid": {"type": "integer"},
                        "reason": {"type": "string"}},
         "required": ["pid", "reason"]},
        quarantine,
    )

    async def echo(request: bytes) -> bytes:
        return request

    builder.register_stream_kind(0x0013, {"type": "object", "properties": {}}, echo)

    builder.trap(TrapFilter.binary("policy-allow"), pre=row_allow)
    builder.trap(TrapFilter.binary("policy-deny"), pre=row_deny)
    builder.trap(TrapFilter.binary("policy-rewrite"), pre=row_rewrite)
    builder.trap(TrapFilter.binary("policy-abort"), pre=row_abort)
    builder.trap(TrapFilter.binary("policy-yield"), pre=row_yield)
    builder.trap(TrapFilter.binary("policy-approval"), pre=row_approval)
    builder.trap(TrapFilter.binary("policy-custom"), pre=row_custom)

    return builder


def test_row_allow_verdict_is_pass() -> None:
    verdict = asyncio.run(row_allow(HandlerContext()))
    assert verdict.kind == "pass"
    assert verdict.payload == {}


def test_row_deny_verdict_is_epolicy_drop() -> None:
    verdict = asyncio.run(row_deny(HandlerContext()))
    assert verdict.kind == "drop"
    assert verdict.payload == {
        "errno": "EPOLICY",
        "reason": "EPOLICY: policy-matrix denies this binary.",
    }


def test_row_rewrite_verdict_patches_execve() -> None:
    verdict = asyncio.run(row_rewrite(HandlerContext()))
    assert verdict.kind == "patch"
    assert verdict.payload["binary"] == "/bin/policy-allow"
    assert verdict.payload["argv"] == ["policy-allow", "--rewritten"]
    assert verdict.payload["env"] == {"KELIR_POLICY_REWRITE": "1"}


def test_row_abort_verdict_is_epolicy_abort() -> None:
    verdict = asyncio.run(row_abort(HandlerContext()))
    assert verdict.kind == "abort"
    assert verdict.payload == {"reason": "EPOLICY: abort row"}


def test_row_yield_gates_on_native_bridge() -> None:
    with pytest.raises(EConfig, match=r"vm checkpoint"):
        asyncio.run(row_yield(HandlerContext()))


def test_row_approval_verdict_suspends_until_deadline() -> None:
    verdict = asyncio.run(row_approval(HandlerContext()))
    assert verdict.kind == "suspend"
    until = verdict.payload["until"]
    assert isinstance(until, Deadline)
    assert until.value == 500


def test_row_custom_verdict_quarantines_trap_pid() -> None:
    verdict = asyncio.run(row_custom(HandlerContext()))
    assert verdict.kind == "patch"
    assert verdict.payload["name"] == "quarantine"
    assert verdict.payload["cbor"] == {
        "pid": 0,
        "reason": "policy-matrix quarantine row",
    }


def test_policy_state_flips_ready_on_vm_ready() -> None:
    assert POLICY_STATE.ready is False
    asyncio.run(PolicyMatrixPlugin().on_vm_ready(None))
    assert POLICY_STATE.ready is True


def test_watchdog_policy_mitigates_on_active_pid() -> None:
    plugin = PolicyMatrixPlugin()
    assert POLICY_STATE.mitigations == 0
    verdict = asyncio.run(plugin.on_watchdog(WatchdogEvent(last_active_pid=7)))
    assert verdict is WatchdogVerdict.Mitigate
    assert POLICY_STATE.mitigations == 1
    verdict = asyncio.run(plugin.on_watchdog(WatchdogEvent()))
    assert verdict is WatchdogVerdict.ExecuteDefault
    assert POLICY_STATE.mitigations == 1


def test_watchdog_silencer_suppresses_only_planned_suspend() -> None:
    silencer = WatchdogSilencerPlugin()
    verdict = asyncio.run(silencer.on_watchdog(WatchdogEvent()))
    assert verdict is WatchdogVerdict.ExecuteDefault
    assert POLICY_STATE.suppressions == 0
    POLICY_STATE.planned_suspend = True
    verdict = asyncio.run(silencer.on_watchdog(WatchdogEvent()))
    assert verdict is WatchdogVerdict.Suppress
    assert POLICY_STATE.suppressions == 1


def test_build_runtime_fuses_policy_matrix() -> None:
    builder = build_runtime()
    assert builder.is_detached() is False
    assert builder.vsock_services == []
    trampoline = builder.fuse()
    assert [chain.ring for chain in trampoline.chains] == [
        RingEvent.LIFECYCLE,
        RingEvent.WATCHDOG,
        RingEvent.CUSTOM_ACTION,
        RingEvent.VSOCK_SERVICE,
        RingEvent.TRAP,
    ]
    assert trampoline.hook_count(RingEvent.LIFECYCLE) == 2
    assert trampoline.hook_count(RingEvent.WATCHDOG) == 2
    assert trampoline.hook_count(RingEvent.CUSTOM_ACTION) == 1
    assert trampoline.hook_count(RingEvent.VSOCK_SERVICE) == 1
    assert trampoline.hook_count(RingEvent.TRAP) == 7
    trap_chain = trampoline.chains[-1]
    binaries = [
        hook.filter.binary
        for hook in trap_chain.hooks
        if hook.filter is not None
    ]
    assert binaries == [
        "policy-allow",
        "policy-deny",
        "policy-rewrite",
        "policy-abort",
        "policy-yield",
        "policy-approval",
        "policy-custom",
    ]
    assert all(hook.stage is HookStage.PRE for hook in trap_chain.hooks)
    described = trampoline.describe()
    assert {ring["ring"]: ring["hook_count"] for ring in described["rings"]} == {
        "lifecycle": 2,
        "watchdog": 2,
        "custom-action": 1,
        "vsock-service": 1,
        "trap": 7,
    }


def test_boot_gate_requires_native_bridge(monkeypatch: pytest.MonkeyPatch) -> None:
    def _missing_bridge() -> Any:
        raise EConfig("kelir native PyO3 extension (_kelir) is missing: install the precompiled wheel (L3)")

    monkeypatch.setattr(kelir, "_load_native_bridge", _missing_bridge)
    with pytest.raises(EConfig, match=r"_kelir\) is missing"):
        asyncio.run(build_runtime().boot())


def test_session_flows_require_native_bridge() -> None:
    session = kelir.Session()
    with pytest.raises(EConfig, match=r"vsock channel"):
        asyncio.run(session.vsock.channel(0x0013))
    with pytest.raises(EConfig, match=r"session spawn"):
        asyncio.run(session.spawn("policy-allow", ["--policy-matrix"]))
    with pytest.raises(EConfig, match=r"session suspend"):
        asyncio.run(session.suspend())
    with pytest.raises(EConfig, match=r"session resume"):
        asyncio.run(session.resume("/tmp/policy-matrix.snap"))
