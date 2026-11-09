// see: cans/02-plugin-model.md#Unified-verdict

import { kelir, createPlugin, t, LifecycleEvent, SuspendUntil, VetoEvent,
         type Session } from "@kelir/core";


const policyState = {
  ready: false,
  plannedSuspend: false,
  mitigations: 0,
  suppressions: 0,
  quarantinedPids: [] as number[],
};

const policyPlugin = createPlugin("policy-matrix")
  .onTrap(async (event, c) => {
    const binary = event.binaryPath.split("/").pop();
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


  .onWatchdog(async (event) => {
    if (event.lastActivePid !== undefined && event.lastActivePid !== null) {
      policyState.mitigations += 1;
      return "mitigate";
    }
    return "execute-default";
  });


const watchdogSilencer = createPlugin("watchdog-silencer")
  .onWatchdog(async () => {
    if (policyState.plannedSuspend) {
      policyState.suppressions += 1;
      return "suppress";
    }
    return "execute-default";
  });


const runtime = kelir({
  vcpus: 2, memoryMb: 2048, driver: "auto",
  kernel: `./fixtures/vmlinux-6.6-${process.arch}-kelir`,
  rootfsErofs: "./fixtures/rootfs-policy.erofs",
  tscLocked: true, enableWatchdog: true,
})
  .use(policyPlugin)
  .use(watchdogSilencer)


  .registerCustomAction("quarantine",
    t.Object({ pid: t.Number(), reason: t.String() }),
    async (payload) => {
      policyState.quarantinedPids.push(payload.pid);
    })


  .registerStreamKind(0x0013, t.Object({}),
    async (request) => request)


  .on(LifecycleEvent.VmInit, async (lifecycle) => {
    lifecycle.registerVeto(VetoEvent.Suspend, () => policyState.ready);
    lifecycle.registerVeto(VetoEvent.ClockSkew, () => policyState.ready);
  })
  .on(LifecycleEvent.VmReady, async () => { policyState.ready = true; });


async function policyMatrixCase_vsockControlEcho(session: Session): Promise<void> {
  const channel = await session.vsock.channel(0x0013);
  await channel.write(new TextEncoder().encode("policy-matrix:ping"));
  const reply = await channel.read();
  console.log(`[kelir] vsock channel 0x0013 reply: ${new TextDecoder().decode(reply)}`);
}

async function policyMatrixCase_trapVerdictRows(session: Session): Promise<void> {
  for (const binary of ["policy-allow", "policy-deny", "policy-rewrite",
                        "policy-abort", "policy-yield", "policy-approval",
                        "policy-custom"]) {
    const run = await session.spawn(binary, ["--policy-matrix"]);
    console.log(`[kelir] ${binary} -> exit=${run.exitCode}`);
  }
}

async function policyMatrixCase_suspendResumeWithWatchdogVeto(session: Session): Promise<void> {
  policyState.plannedSuspend = true;
  await session.suspend("/tmp/policy-matrix.snap");
  await session.resume("/tmp/policy-matrix.snap");
  policyState.plannedSuspend = false;
}

async function main(): Promise<void> {
  const session: Session = await runtime.boot();
  console.log(`[kelir] driver=${session.driverName} boot=${session.bootMs}ms`);

  await policyMatrixCase_vsockControlEcho(session);
  await policyMatrixCase_trapVerdictRows(session);
  await policyMatrixCase_suspendResumeWithWatchdogVeto(session);

  await session.terminate();
}

main().catch(console.error);
