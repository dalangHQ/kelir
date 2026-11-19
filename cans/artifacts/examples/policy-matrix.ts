// see: cans/02-plugin-model.md#Unified-verdict

import { kelir, createPlugin, t, SuspendUntil, SuspendReason, VetoEvent, Errno,
         type Session } from "@kelir/core";

const policyState = {
  ready: false,
  plannedSuspend: false,
  mitigations: 0,
  suppressions: 0,
  quarantinedPids: [] as number[],
};

const policyPlugin = createPlugin("policy-matrix")
  .trap((t) =>
    t.onTrap(async (event, c) => {
      const binary = event.binaryPath.split("/").pop();
      switch (binary) {
        case "policy-allow":
          return c.pass();
        case "policy-deny":
          // errno is the closed Errno enum; stringly errnos are non-conforming.
          return c.drop(Errno.EPOLICY, "EPOLICY: policy-matrix denies this binary.");
        // see: cans/02-plugin-model.md#Patch-payload-is-registered-per-ring:-ExecveMutation,-FsInode,
        case "policy-rewrite":
          return c.patch({
            binary: "/bin/policy-allow",
            argv: ["policy-allow", "--rewritten"],
            env: { KELIR_POLICY_REWRITE: "1" },
          });
        case "policy-abort":
          return c.abort("EPOLICY: abort row");
        case "policy-yield":
          await c.checkpoint.create("policy-yield");
          return c.suspend(SuspendUntil.approval);
        case "policy-approval":
          return c.suspend(SuspendUntil.deadline(500));
        case "policy-escalate":
          if (c.hasValidOperatorLease()) {
            return c.pass();
          }
          return c.drop(Errno.EPERM, "ESECURITYVIOLATION: capability widening requires signed operator lease");
        case "policy-syscall-block":
          return c.drop(Errno.EAFNOSUPPORT, "EAFNOSUPPORT: socket family AF_RXRPC blocked");
        // see: cans/11-protocol.md#Custom-payload:-c.patch(Custom-{-name,-cbor-})
        case "policy-custom":
          return c.patch({
            name: "quarantine",
            cbor: {
              pid: c.trap.pid,
              reason: "policy-matrix quarantine row",
            },
          });
        default:
          return c.pass();
      }
    }),
  )
  // see: cans/08-lifecycle.md#Watchdog
  .telemetry((tm) =>
    tm.onWatchdog(async (event) => {
      if (event.lastActivePid !== undefined && event.lastActivePid !== null) {
        policyState.mitigations += 1;
        return "mitigate";
      }
      return "execute-default";
    }),
  );

const watchdogSilencer = createPlugin("watchdog-silencer")
  .telemetry((tm) =>
    tm.onWatchdog(async () => {
      if (policyState.plannedSuspend) {
        policyState.suppressions += 1;
        return "suppress";
      }
      return "execute-default";
    }),
  );

async function main() {
  const session: Session = await kelir({
    vcpus: 2, memoryMb: 2048, driver: "auto",
    kernel: `./fixtures/vmlinux-6.6-${process.arch}-kelir`,
    rootfsErofs: "./fixtures/rootfs-policy.erofs",
    tscLocked: true, enableWatchdog: true,
  })
    .use(policyPlugin)
    .use(watchdogSilencer)
    // see: cans/11-protocol.md#Unknown-name-at-trap-time-fails-with-EUnknownAction
    .actions((a) =>
      a.register(
        "quarantine",
        t.Object({ pid: t.Number(), reason: t.String() }),
        async (payload) => {
          policyState.quarantinedPids.push(payload.pid);
        },
      ),
    )
    .vsock((v) => v.service(0x0013, "echo", async (request) => request))
    // see: cans/08-lifecycle.md#Lifecycle-vetoes
    .lifecycle((l) =>
      l
        .onVmInit(async (lifecycle) => {
          lifecycle.registerVeto(VetoEvent.Suspend, () => policyState.ready);
          lifecycle.registerVeto(VetoEvent.ClockSkew, () => policyState.ready);
        })
        .onVmReady(async () => {
          policyState.ready = true;
        }),
    )
    .seal()
    .boot();
  console.log(`[kelir] driver=${session.driverName} boot=${session.bootMs}ms`);

  // see: cans/03-sdk-architecture.md#Multiplexed-binary-RPC-for-SDK-consumers:-session.vsock.channel(kind),-binary-framing
  const channel = await session.vsock.channel(0x0013);
  await channel.write(new TextEncoder().encode("policy-matrix:ping"));
  const reply = await channel.read();
  console.log(`[kelir] vsock channel 0x0013 reply: ${new TextDecoder().decode(reply)}`);

  for (const binary of ["policy-allow", "policy-deny", "policy-rewrite",
                        "policy-abort", "policy-yield", "policy-approval",
                        "policy-escalate", "policy-syscall-block", "policy-custom"]) {
    const run = await session.spawn(binary, ["--policy-matrix"]);
    console.log(`[kelir] ${binary} -> exit=${run.exitCode}`);
    if (binary === "policy-approval") {
      if (run.exitCode !== 0) throw new Error("approval failed");
    } else if (binary === "policy-escalate") {
      if (run.exitCode === 0) throw new Error("escalate should fail without signed lease");
      await session.elevateLease("signed_jws_operator_lease_token_payload");
      const elevated = await session.spawn(binary, ["--policy-matrix"]);
      if (elevated.exitCode !== 0) throw new Error("escalation failed post-signature");
    } else if (binary === "policy-syscall-block") {
      if (run.exitCode === 0) throw new Error("syscall block should fail execution");
    } else if (binary === "policy-custom") {
      if (run.exitCode !== 0) throw new Error("custom failed");
    }

    const receipt = run.receipt;
    if (!receipt || receipt.wallClockNs === 0) throw new Error("missing raw hardware receipt");
  }

  policyState.plannedSuspend = true;
  await session.suspend(SuspendReason.resourcePressure);
  await session.resume();
  policyState.plannedSuspend = false;

  await session.terminate();
}

main().catch(console.error);
