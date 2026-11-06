
from __future__ import annotations

import asyncio
import platform
from dataclasses import dataclass, field
from typing import Any, Awaitable, Callable

import kelir
from kelir import Approval, Deadline, HandlerContext, KelirBuilder, TrapFilter, Verdict
from kelir.plugin import (
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
        driver="auto",
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

    builder.register_vsock_service(0x0013, {"type": "object", "properties": {}}, echo)

    builder.trap(TrapFilter.binary("policy-allow"), pre=row_allow)
    builder.trap(TrapFilter.binary("policy-deny"), pre=row_deny)
    builder.trap(TrapFilter.binary("policy-rewrite"), pre=row_rewrite)
    builder.trap(TrapFilter.binary("policy-abort"), pre=row_abort)
    builder.trap(TrapFilter.binary("policy-yield"), pre=row_yield)
    builder.trap(TrapFilter.binary("policy-approval"), pre=row_approval)
    builder.trap(TrapFilter.binary("policy-custom"), pre=row_custom)

    return builder


async def main() -> None:
    async with build_runtime() as session:
        print(f"[kelir] booted driver={session.driver_name} "
              f"boot_ms={session.boot_ms}")

        channel = await session.vsock.channel(0x0013)
        await channel.write(b"policy-matrix:ping")
        reply = await channel.read()
        print(f"[kelir] vsock channel 0x0013 reply: {reply!r}")

        for binary in ["policy-allow", "policy-deny", "policy-rewrite",
                       "policy-abort", "policy-yield", "policy-approval",
                       "policy-custom"]:
            run = await session.spawn(binary, ["--policy-matrix"])
            print(f"[kelir] {binary} -> exit={run.exit_code}")

        POLICY_STATE.planned_suspend = True
        await session.suspend("/tmp/policy-matrix.snap")
        await session.resume("/tmp/policy-matrix.snap")
        POLICY_STATE.planned_suspend = False


if __name__ == "__main__":
    asyncio.run(main())
