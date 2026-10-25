// ============================================================================
// policy-matrix.ts
// ============================================================================
// Canonical depth example: the full TrapAction lattice, lifecycle vetoes,
// watchdog verdicts, a custom action, and binary vsock RPC — the surface
// the swe-agent family deliberately does not touch.
//
// SAME CHAIN as policy-matrix.rs and policy-matrix.py. Registration order
// is the dispatch order; each binary hits exactly one row:
//
//   1. policy-allow    -> Allow            (resume untouched)
//   2. policy-deny     -> Deny            (exit 1, EPOLICY)
//   3. policy-rewrite  -> Rewrite         (new binary + argv, env ONLY via
//                                            Rewrite env — never env.set()
//                                            + resume, which is
//                                            EINVALIDDECISION per 09)
//   4. policy-abort    -> Abort           (pre-exec, exit 1)
//   5. policy-yield    -> checkpoint "policy-yield", then Yield to host
//   6. policy-approval -> SuspendForApproval (poll 500 ms)
//   7. policy-custom   -> Custom("quarantine", payload) dispatched to
//                          registerCustomAction("quarantine", …)
//   8. Vetoes (vm:init): Suspend and ClockSkew, each allowed only once
//      the ready flag is set in vm:ready; before that both abort.
//   9. Watchdog: the policy plugin returns Mitigate when a live PID was
//      active at fire time; the silencer plugin returns Suppress while a
//      planned suspend is in progress, else ExecuteDefault.
//  10. Vsock: registerVsockService(2, …) + session.vsock.channel(2)
//      binary RPC round-trip in main().
//
// SDK-IDIOM NOTE: TS has no plugin on_trap surface in any example, so
// this file expresses the same table through builder .trap rows (the
// documented TS trap mechanism, 03), while Rust/Python express it
// through the plugin's on_trap. Decision sequence and order are
// identical; only the registration mechanism follows SDK idiom.
// Shared veto/watchdog state lives in a module-level object closed over
// by the plugin factories, the same closure pattern the swe-agent TS
// plugin uses for its options.
//
// GAPS (flagged, not silently invented):
//   - c.abort / c.yield / c.suspendForApproval / c.custom: 09 maps the
//     lattice to c.abort, c.yield(checkpoint_id),
//     c.suspend_for_approval(poll_ms), c.custom(name, cbor_payload).
//     Only c.resume / c.deny / c.rewrite appear in existing TS examples;
//     the four new spellings below are 09's names in TS camelCase idiom
//     (cf. warmCache, execInGuest), not spellings shown in any example.
//   - Yield id: TrapAction::Yield checkpoint_id is String, matching
//     SnapshotDescriptor.id, so this file yields with the "policy-yield"
//     label.
//   - Veto registration in TS: no veto spelling appears in any TS
//     example or in 03's .on() list (vm:init, vm:ready, vm:shutdown,
//     trap:deny). The registerVeto calls in the vm:init handler mirror
//     LifecycleCtx.register_veto (02/03) in TS idiom; event names use
//     the spec VetoEvent names kebab-cased.
//   - Watchdog hook in TS: no on_watchdog spelling appears in any TS
//     example. The createPlugin .onWatchdog hooks below mirror the Rust
//     trait name in TS camelCase idiom.
//   - registerCustomAction / registerVsockService: specified in 09/03
//     but handler signatures and schema types appear in no example;
//     the TypeBox schemas and handlers below are the closest documented
//     form.
//   - session.vsock.channel(2) is specified in 03; the channel's
//     read/write shape is not shown anywhere. write/read below are the
//     closest generic duplex form.

import { kelir, createPlugin, t,
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
  // Unknown names fail with EUnknownAction (09).
  .registerCustomAction("quarantine",
    t.Object({ pid: t.Number(), reason: t.String() }),
    async (payload) => {
      policyState.quarantinedPids.push(payload.pid);
    })

  // Vsock service on port 2: the RPC peer of session.vsock.channel(2)
  // in main(). Echoes the request frame back to the caller.
  .registerVsockService(2, t.Object({}),
    async (request) => request)

  // Row 1: Allow.
  .trap("policy-allow", async (c) => c.resume())
  // Row 2: Deny.
  .trap("policy-deny", async (c) =>
    c.deny(1, "EPOLICY: policy-matrix denies this binary."))
  // Row 3: Rewrite — env ONLY through the rewrite payload.
  .trap("policy-rewrite", async (c) =>
    c.rewrite({ binary: "/bin/policy-allow",
                argv: ["policy-allow", "--rewritten"],
                env: { KELIR_POLICY_REWRITE: "1" } }))
  // Row 4: Abort, pre-exec.
  .trap("policy-abort", async (c) =>
    c.abort(1, "EPOLICY: abort row"))
  // Row 5: Yield — checkpoint first, then freeze to the host.
  .trap("policy-yield", async (c) => {
    await c.vm.checkpoint("policy-yield");
    return c.yield("policy-yield");
  })
  // Row 6: SuspendForApproval — host polls every 500 ms.
  .trap("policy-approval", async (c) => c.suspendForApproval(500))
  // Row 7: Custom — dispatched by name to the handler above.
  .trap("policy-custom", async (c) =>
    c.custom("quarantine", { pid: c.trap.pid,
                             reason: "policy-matrix quarantine row" }))

  // Vetoes (row 8): registered at vm:init, mirroring
  // LifecycleCtx.register_veto. Both allow only once vm:ready has set
  // the ready flag; before that Suspend / ClockSkew abort (07).
  .on("vm:init", async (lifecycle) => {
    lifecycle.registerVeto("suspend", () => policyState.ready);
    lifecycle.registerVeto("clock-skew", () => policyState.ready);
  })
  .on("vm:ready", async () => { policyState.ready = true; });

// ============================================================================
// BOOT — drive every row in chain order
// ============================================================================

async function main() {
  const session: Session = await runtime.boot();
  console.log(`[kelir] driver=${session.driverName} boot=${session.bootMs}ms`);

  // Vsock binary RPC (03: session.vsock.channel(2), binary framing).
  const channel = await session.vsock.channel(2);
  await channel.write(new TextEncoder().encode("policy-matrix:ping"));
  const reply = await channel.read();
  console.log(`[kelir] vsock channel 2 reply: ${new TextDecoder().decode(reply)}`);

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
