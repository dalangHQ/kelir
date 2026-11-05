// ============================================================================
// policy-matrix.ts
// ============================================================================
// Canonical depth example: the full Verdict lattice, lifecycle vetoes,
// watchdog verdicts, a custom action, and binary vsock RPC — the surface
// the swe-agent family deliberately does not touch.
//
// SAME CHAIN as policy-matrix.rs and policy-matrix.py. Registration order
// is the dispatch order; each binary hits exactly one row:
//
//   1. policy-allow    -> Pass            (proceed untouched)
//   2. policy-deny     -> Drop           (errno EPOLICY, reason)
//   3. policy-rewrite  -> Patch          (new binary + argv, env ONLY via
//                                            the ExecveMutation payload
//                                            — never env.set() + pass,
//                                            which is EINVALIDDECISION
//                                            per 09)
//   4. policy-abort    -> Abort           (pre-exec, reason only)
//   5. policy-yield    -> checkpoint "policy-yield" taken first, then
//                          Suspend{until: Approval}
//   6. policy-approval -> Suspend        (until Deadline(500))
//   7. policy-custom   -> Patch(Custom{name, cbor}) dispatched to
//                          registerCustomAction("quarantine", …)
//   8. Vetoes (vm:init): Suspend and ClockSkew, each allowed only once
//      the ready flag is set in vm:ready; before that both abort.
//   9. Watchdog: the policy plugin returns Mitigate when a live PID was
//      active at fire time; the silencer plugin returns Suppress while a
//      planned suspend is in progress, else ExecuteDefault.
//  10. Vsock: registerVsockService(0x0013, …) + session.vsock.channel(0x0013)
//      binary RPC round-trip in main().
//
// NOTES:
//   - Lattice methods: c.pass / c.patch / c.drop / c.suspend / c.abort.
//   - Suspend{until: Approval}: checkpoint taken first, then suspend
//     awaiting host approval/external input (see row 5).
//   - Vetoes register at vm:init via lifecycle.registerVeto(event, cb);
//     events are typed VetoEvent members.
//   - Watchdog hooks via createPlugin().onWatchdog — TS camelCase of
//     SystemTelemetryParticipant::on_watchdog.
//   - registerCustomAction / registerVsockService take TypeBox schema
//     and an async handler, mirroring 09/03.
//   - session.vsock.channel(kind) is an async duplex byte stream.

import { kelir, createPlugin, t, SuspendUntil, VetoEvent,
         type Session } from "@kelir/core";

// ── Shared policy state ───────────────────────────────────────────────────
// The ready flag gates both vetoes; plannedSuspend gates Suppress.
const policyState = {
  ready: false,
  plannedSuspend: false,
  mitigations: 0,
  suppressions: 0,
  quarantinedPids: [] as number[],
};

const policyPlugin = createPlugin("policy-matrix")
  .onTrap(async (event, c) => {
    const binary = event.binary_path.split("/").pop();
    switch (binary) {
      case "policy-allow":
        return c.pass();
      case "policy-deny":
        return c.drop("EPOLICY", "EPOLICY: policy-matrix denies this binary.");
      case "policy-rewrite":
        return c.patch({
          binary: "/bin/policy-allow",
          argv: ["policy-allow", "--rewritten"],
          env: { KELIR_POLICY_REWRITE: "1" },
        });
      case "policy-abort":
        return c.abort("EPOLICY: abort row");
      case "policy-yield":
        await c.vm.checkpoint("policy-yield");
        return c.suspend(SuspendUntil.approval);
      case "policy-approval":
        return c.suspend(SuspendUntil.deadline(500));
      case "policy-custom":
        return c.patch({
          name: "quarantine",
          cbor: {
            pid: c.trap.pid,
            reason: "policy-matrix quarantine row",
          },
        });
      default:
        return null;
    }
  })
  // Watchdog, policy half: a live PID was active at fire time, so do
  // not run the default Reboot/Terminate action — delegate recovery to
  // the host policy handler (07: Mitigate delegates recovery to host).
  .onWatchdog(async (event) => {
    if (event.lastActivePid !== undefined && event.lastActivePid !== null) {
      policyState.mitigations += 1;
      return "mitigate";
    }
    return "execute-default";
  });

// Second watchdog plugin: Suppress is a real second code path, not a
// comment. While main() holds a planned suspend open, a watchdog firing
// inside that window must not reboot an intentionally parked VM.
const watchdogSilencer = createPlugin("watchdog-silencer")
  .onWatchdog(async () => {
    if (policyState.plannedSuspend) {
      policyState.suppressions += 1;
      return "suppress";
    }
    return "execute-default";
  });

// ============================================================================
// SINGLE LINEAR CHAIN — trap rows in dispatch order, then lifecycle
// ============================================================================

const runtime = kelir({
  vcpus: 2, memoryMb: 2048, driver: "auto",
  kernel: `./fixtures/vmlinux-6.6-${process.arch}-kelir`,
  rootfsErofs: "./fixtures/rootfs-policy.erofs",
  tscLocked: true, enableWatchdog: true,
})
  .use(policyPlugin)
  .use(watchdogSilencer)

  // Custom action "quarantine": row 7 dispatches here by name.
  // Unknown names fail with EUnknownAction (11).
  .registerCustomAction("quarantine",
    t.Object({ pid: t.Number(), reason: t.String() }),
    async (payload) => {
      policyState.quarantinedPids.push(payload.pid);
    })

  // Vsock service on plugin kind 0x0013: the RPC peer of session.vsock.channel(0x0013)
  // in main(). Echoes the request frame back to the caller.
  .registerVsockService(0x0013, t.Object({}),
    async (request) => request)


  // Vetoes (row 8): registered at vm:init, mirroring
  // LifecycleCtx.register_veto. Both allow only once vm:ready has set
  // the ready flag; before that Suspend / ClockSkew abort (08).
  .on(LifecycleEvent.VmInit, async (lifecycle) => {
    lifecycle.registerVeto(VetoEvent.Suspend, () => policyState.ready);
    lifecycle.registerVeto(VetoEvent.ClockSkew, () => policyState.ready);
  })
  .on(LifecycleEvent.VmReady, async () => { policyState.ready = true; });

// ============================================================================
// BOOT — drive every row in chain order
// ============================================================================

async function main() {
  const session: Session = await runtime.boot();
  console.log(`[kelir] driver=${session.driverName} boot=${session.bootMs}ms`);

  // Vsock binary RPC (03: session.vsock.channel(0x0013), binary framing).
  const channel = await session.vsock.channel(0x0013);
  await channel.write(new TextEncoder().encode("policy-matrix:ping"));
  const reply = await channel.read();
  console.log(`[kelir] vsock channel 0x0013 reply: ${new TextDecoder().decode(reply)}`);

  // Rows 1-3 return to the guest; rows 4-7 park, abort, or divert.
  // Each spawn exercises exactly one trap row, in chain order.
  for (const binary of ["policy-allow", "policy-deny", "policy-rewrite",
                        "policy-abort", "policy-yield", "policy-approval",
                        "policy-custom"]) {
    const run = await session.spawn(binary, ["--policy-matrix"]);
    console.log(`[kelir] ${binary} -> exit=${run.exitCode}`);
  }

  // Planned suspend window: a watchdog firing here hits the silencer's
  // Suppress path instead of the default Reboot/Terminate action.
  policyState.plannedSuspend = true;
  await session.suspend("/tmp/policy-matrix.snap");
  await session.resume("/tmp/policy-matrix.snap");
  policyState.plannedSuspend = false;

  await session.terminate();
}

main().catch(console.error);
