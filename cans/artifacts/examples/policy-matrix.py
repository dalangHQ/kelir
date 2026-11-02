# examples/policy-matrix.py
"""
Canonical depth example: the full Verdict lattice, lifecycle vetoes,
watchdog verdicts, a custom action, and binary vsock RPC — the surface
the swe-agent family deliberately does not touch.

SAME CHAIN as policy-matrix.rs and policy-matrix.ts. Registration order
is the dispatch order; each binary hits exactly one row:

  1. policy-allow    -> Pass            (proceed untouched)
  2. policy-deny     -> Drop           (errno EPOLICY, reason)
  3. policy-rewrite  -> Patch          (new binary + argv, env ONLY via
                                           the ExecveMutation payload
                                           — never env.set() + pass,
                                           which is EINVALIDDECISION
                                           per 09)
  4. policy-abort    -> Abort           (pre-exec, reason only)
  5. policy-yield    -> checkpoint "policy-yield" taken first, then
                         Suspend{until: Approval}
  6. policy-approval -> Suspend        (until Deadline(500))
  7. policy-custom   -> Patch(Custom{name, cbor}) dispatched to
                         builder.register_custom_action("quarantine", …)
  8. Vetoes (on_vm_init): Suspend and ClockSkew, each allowed only once
     the ready flag is set in on_vm_ready; before that both abort.
  9. Watchdog: the policy plugin returns Mitigate when a live PID was
     active at fire time; the silencer plugin returns Suppress while a
     planned suspend is in progress, else ExecuteDefault.
 10. Vsock: builder.register_vsock_service(0x0013, …) +
     session.vsock.channel(0x0013) binary RPC round-trip in main().

SDK-IDIOM NOTE: two equivalent trap surfaces exist in every SDK —
plugin.on_trap (Rust, Python) / plugin.onTrap (TS), and
builder.trap() (all three). Both register into the same pipeline.
This file uses builder.trap() because a lattice-table example reads
as a table; the plugin.on_trap surface is exercised in
hook-matrix.py.

NOTES:
  - Lattice: Verdict.pass_ / patch / drop / suspend / abort.
  - Suspend{until: Approval}: checkpoint taken first, then suspend
    awaiting host approval/external input (see row 5).
  - Vetoes register at on_vm_init via ctx.register_veto(event, cb).
  - Watchdog via on_watchdog on BasePlugin
    (SystemTelemetryParticipant in snake_case).
  - register_custom_action / register_vsock_service take a dict
    schema and an async handler.
  - session.vsock.channel(kind) is an async duplex byte stream.
  - c.vm.checkpoint is available on HandlerContext in all SDKs.
"""

from __future__ import annotations

import asyncio
import platform
from dataclasses import dataclass, field
from typing import Awaitable, Callable

import kelir
from kelir import Approval, Deadline, HandlerContext, KelirBuilder, TrapFilter, Verdict
from kelir.plugin import (
    BasePlugin,
    VetoEvent,
    WatchdogEvent,
    WatchdogVerdict,
)


# ============================================================================
# 1. SHARED STATE — veto flags, watchdog counters, quarantine log
# ============================================================================

@dataclass
class PolicyState:
    ready: bool = False
    planned_suspend: bool = False
    mitigations: int = 0
    suppressions: int = 0
    quarantined_pids: list[int] = field(default_factory=list)


POLICY_STATE = PolicyState()


# ============================================================================
# 2. PLUGINS — vetoes + watchdog verdicts (the trap lattice is in §4)
# ============================================================================

# Plugin: policy-matrix; rings: virtio-vsock, virtio-fs
class PolicyMatrixPlugin(BasePlugin):
    name = "policy-matrix"

    async def on_vm_init(self, ctx) -> None:
        # Spec form (02: LifecycleCtx exposes register_veto). Both vetoes
        # allow only once on_vm_ready has set the ready flag; before that
        # any Suspend or ClockSkew aborts with ESessionVeto (08).
        ctx.register_veto(VetoEvent.Suspend, lambda: POLICY_STATE.ready)
        ctx.register_veto(VetoEvent.ClockSkew, lambda: POLICY_STATE.ready)

    async def on_vm_ready(self, _ctx) -> None:
        POLICY_STATE.ready = True

    async def on_watchdog(self, event: WatchdogEvent) -> WatchdogVerdict:
        # A live PID was active when the watchdog fired: do not run the
        # default Reboot/Terminate action; delegate recovery to the host
        # policy handler (07: Mitigate delegates recovery to host).
        if event.last_active_pid is not None:
            POLICY_STATE.mitigations += 1
            return WatchdogVerdict.Mitigate
        return WatchdogVerdict.ExecuteDefault


class WatchdogSilencerPlugin(BasePlugin):
    """Suppress is a real second code path, not a comment. While main()
    holds a planned suspend open, a watchdog firing inside that window
    must not reboot an intentionally parked VM."""

    name = "watchdog-silencer"

    async def on_watchdog(self, _event: WatchdogEvent) -> WatchdogVerdict:
        if POLICY_STATE.planned_suspend:
            POLICY_STATE.suppressions += 1
            return WatchdogVerdict.Suppress
        return WatchdogVerdict.ExecuteDefault


# ============================================================================
# 3. TRAP ROWS — one pre-handler per binary, in dispatch order
# ============================================================================

PreExecHandler = Callable[[HandlerContext], Awaitable[Verdict]]


async def row_allow(c: HandlerContext) -> Verdict:
    return Verdict.pass_()


async def row_deny(c: HandlerContext) -> Verdict:
    return Verdict.drop("EPOLICY", "EPOLICY: policy-matrix denies this binary.")


async def row_rewrite(c: HandlerContext) -> Verdict:
    # Env mutation ONLY through the patch (ExecveMutation) payload.
    return Verdict.patch(
        binary="/bin/policy-allow",
        argv=["policy-allow", "--rewritten"],
        env={"KELIR_POLICY_REWRITE": "1"},
    )


async def row_abort(c: HandlerContext) -> Verdict:
    return Verdict.abort("EPOLICY: abort row")


# Suspend{until: Approval}: checkpoint taken first, then suspend
# awaiting host approval/external input.
async def row_yield(c: HandlerContext) -> Verdict:
    await c.vm.checkpoint("policy-yield")
    return Verdict.suspend(until=Approval)


async def row_approval(c: HandlerContext) -> Verdict:
    return Verdict.suspend(until=Deadline(500))


async def row_custom(c: HandlerContext) -> Verdict:
    # Dispatched by name to the handler registered on the builder below.
    # Unknown names fail with EUnknownAction (11).
    return Verdict.patch(
        name="quarantine",
        cbor={"pid": c.trap.pid, "reason": "policy-matrix quarantine row"},
    )


# ============================================================================
# 4. BUILDER — plugins, custom action, vsock service, trap rows
# ============================================================================

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

    # Custom action "quarantine": row 7 dispatches here by name. The
    # handler records the pid; the quarantine snapshot itself is taken
    # by main() (a handler receives no Session per 02's hook context
    # restriction).
    async def quarantine(payload: dict) -> None:
        POLICY_STATE.quarantined_pids.append(payload["pid"])

    builder.register_custom_action(
        "quarantine",
        {"type": "object",
         "properties": {"pid": {"type": "integer"},
                        "reason": {"type": "string"}},
         "required": ["pid", "reason"]},
        quarantine,
    )

    # Vsock service on plugin kind 0x0013: the RPC peer of session.vsock.channel(0x0013)
    # in main(). Echoes the request frame back to the caller.
    async def echo(request: bytes) -> bytes:
        return request

    builder.register_vsock_service(0x0013, {"type": "object", "properties": {}}, echo)

    # Trap rows, in dispatch order. Each pattern hits exactly one row.
    builder.trap(TrapFilter.binary("policy-allow"), pre=row_allow)
    builder.trap(TrapFilter.binary("policy-deny"), pre=row_deny)
    builder.trap(TrapFilter.binary("policy-rewrite"), pre=row_rewrite)
    builder.trap(TrapFilter.binary("policy-abort"), pre=row_abort)
    builder.trap(TrapFilter.binary("policy-yield"), pre=row_yield)
    builder.trap(TrapFilter.binary("policy-approval"), pre=row_approval)
    builder.trap(TrapFilter.binary("policy-custom"), pre=row_custom)

    return builder


# ============================================================================
# 5. MAIN — drive every row in chain order
# ============================================================================

async def main() -> None:
    async with build_runtime() as session:
        print(f"[kelir] booted driver={session.driver_name} "
              f"boot_ms={session.boot_ms}")

        # Vsock binary RPC (03: session.vsock.channel(0x0013), binary framing).
        channel = await session.vsock.channel(0x0013)
        await channel.write(b"policy-matrix:ping")
        reply = await channel.read()
        print(f"[kelir] vsock channel 0x0013 reply: {reply!r}")

        # Rows 1-3 return to the guest; rows 4-7 park, abort, or divert.
        # Each spawn exercises exactly one trap row, in chain order.
        for binary in ["policy-allow", "policy-deny", "policy-rewrite",
                       "policy-abort", "policy-yield", "policy-approval",
                       "policy-custom"]:
            run = await session.spawn(binary, ["--policy-matrix"])
            print(f"[kelir] {binary} -> exit={run.exit_code}")

        # Planned suspend window: a watchdog firing here hits the
        # silencer's Suppress path instead of the default action.
        POLICY_STATE.planned_suspend = True
        await session.suspend("/tmp/policy-matrix.snap")
        await session.resume("/tmp/policy-matrix.snap")
        POLICY_STATE.planned_suspend = False


if __name__ == "__main__":
    asyncio.run(main())
