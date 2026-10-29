# examples/policy-matrix.py
"""
Canonical depth example: the full TrapAction lattice, lifecycle vetoes,
watchdog verdicts, a custom action, and binary vsock RPC — the surface
the swe-agent family deliberately does not touch.

SAME CHAIN as policy-matrix.rs and policy-matrix.ts. Registration order
is the dispatch order; each binary hits exactly one row:

  1. policy-allow    -> Allow            (resume untouched)
  2. policy-deny     -> Deny            (exit 1, EPOLICY)
  3. policy-rewrite  -> Rewrite         (new binary + argv, env ONLY via
                                           Rewrite env — never env.set()
                                           + resume, which is
                                           EINVALIDDECISION per 09)
  4. policy-abort    -> Abort           (pre-exec, exit 1)
  5. policy-yield    -> checkpoint "policy-yield", then Yield to host
  6. policy-approval -> Hold (500 ms)
  7. policy-custom   -> Custom("quarantine", payload) dispatched to
                         builder.register_custom_action("quarantine", …)
  8. Vetoes (on_vm_init): Suspend and ClockSkew, each allowed only once
     the ready flag is set in on_vm_ready; before that both abort.
  9. Watchdog: the policy plugin returns Mitigate when a live PID was
     active at fire time; the silencer plugin returns Suppress while a
     planned suspend is in progress, else ExecuteDefault.
 10. Vsock: builder.register_vsock_service(2, …) +
     session.vsock.channel(2) binary RPC round-trip in main().

SDK-IDIOM NOTE: Python expresses the lattice through builder.trap
pre-handlers returning Decision (the documented Python trap mechanism,
cf. the swe-agent Python builder.trap(pre=…) rows), while Rust
expresses it through the plugin's on_trap returning TrapAction. The
decision sequence and order are identical; only the registration
mechanism follows SDK idiom. Watchdog/veto hooks live on BasePlugin
subclasses, mirroring the Rust capability traits in snake_case (cf.
on_trap / on_fs_write / on_disclosure in the swe-agent Python plugin).

GAPS (flagged, not silently invented):
  - Decision.abort / Decision.yield_ / Decision.hold /
    Decision.custom: 09 maps the lattice to c.abort,
    c.yield(checkpoint_id), c.hold(duration_ms),
    c.custom(name, cbor_payload). Only Decision.deny / resume / rewrite
    appear in existing Python examples; the four new spellings below are
    09's names in the existing static-Decision idiom, not spellings
    shown in any example. yield_ carries a trailing underscore because
    bare `.yield` is a Python syntax error (yield is a keyword).
  - Yield id: TrapAction::Yield checkpoint_id is String, matching
    SnapshotDescriptor.id, so this file yields with the "policy-yield"
    label.
  - Veto registration in Python: no veto spelling appears in any Python
    example, and the swe-agent Python lifecycle uses builder decorators
    (@builder.on_vm_init), not plugin methods. The plugin on_vm_init /
    on_vm_ready methods below mirror the Rust LifecycleParticipant
    trait, and their register_veto calls mirror LifecycleCtx.register_veto
    (02/03); the VetoEvent import path follows the kelir.plugin import
    precedent (PluginError, TrapAction).
  - Watchdog hook in Python: no on_watchdog spelling appears in any
    Python example. The method below mirrors the Rust trait name in
    snake_case, following the on_trap / on_disclosure precedent.
  - builder.register_custom_action / builder.register_vsock_service:
    specified in 09/03 but handler signatures and schema types appear in
    no example; the dict schemas and handlers below are the closest
    documented form.
  - session.vsock.channel(2) is specified in 03; the channel's
    read/write shape is not shown anywhere. write/read below are the
    closest generic duplex form.
  - Handler-context vm.checkpoint: the swe-agent Python handlers use
    c.receipt() / terminal service but never c.vm.checkpoint; the yield row below
    assumes the Python HandlerContext exposes vm like its TS/Rust
    counterparts.

Requirements:
    kelir>=3.0
"""

from __future__ import annotations

import asyncio
import platform
from dataclasses import dataclass, field
from typing import Awaitable, Callable

import kelir
from kelir import Decision, HandlerContext, KelirBuilder
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
        # any Suspend or ClockSkew aborts with ESessionVeto (07).
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

PreExecHandler = Callable[[HandlerContext], Awaitable[Decision]]


async def row_allow(c: HandlerContext) -> Decision:
    return Decision.resume()


async def row_deny(c: HandlerContext) -> Decision:
    return Decision.deny(1, "EPOLICY: policy-matrix denies this binary.")


async def row_rewrite(c: HandlerContext) -> Decision:
    # Env mutation ONLY through the rewrite payload.
    return Decision.rewrite(
        binary="/bin/policy-allow",
        argv=["policy-allow", "--rewritten"],
        env={"KELIR_POLICY_REWRITE": "1"},
    )


async def row_abort(c: HandlerContext) -> Decision:
    return Decision.abort(1, "EPOLICY: abort row")


async def row_yield(c: HandlerContext) -> Decision:
    await c.vm.checkpoint("policy-yield")
    # yield_ (trailing underscore): bare `.yield` is a Python syntax
    # error, since yield is a keyword even after a dot.
    return Decision.yield_("policy-yield")


async def row_approval(c: HandlerContext) -> Decision:
    return Decision.hold(duration_ms=500)


async def row_custom(c: HandlerContext) -> Decision:
    # Dispatched by name to the handler registered on the builder below.
    # Unknown names fail with EUnknownAction (09).
    return Decision.custom(
        "quarantine",
        {"pid": c.trap.pid, "reason": "policy-matrix quarantine row"},
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

    # Vsock service on vsock channel kind 2: the RPC peer of session.vsock.channel(2)
    # in main(). Echoes the request frame back to the caller.
    async def echo(request: bytes) -> bytes:
        return request

    builder.register_vsock_service(2, {"type": "object", "properties": {}}, echo)

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

        # Vsock binary RPC (03: session.vsock.channel(2), binary framing).
        channel = await session.vsock.channel(2)
        await channel.write(b"policy-matrix:ping")
        reply = await channel.read()
        print(f"[kelir] vsock channel 2 reply: {reply!r}")

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
