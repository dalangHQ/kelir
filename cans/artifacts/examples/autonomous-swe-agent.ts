// see: cans/03-sdk-architecture.md#TypeScript-SDK

import { kelir, createPlugin, t, TrapFilter, PostExecAction, SuspendReason, FilterAction, Verdict, PatchPayload,
         type KelirBuilder, type ExecutionReceipt,
         type CausalDelta, type PolicyHandle, type LedgerHandle, type Infer } from "@kelir/core";
import { chaos, ledger, redaction, terminalGuard, tokenShield, vault } from "@kelir/core";

declare const oracle: {
  currentTraceId(): string;
  analyzeIntent(trap: { binary: string; binaryBlake3: string; argv: string[];
                        lineage: number; cwd: string }): Promise<{ isHypothesisValid: boolean;
                                                          falsificationReason?: string }>;
  verifyPhysicalReceipt(r: ExecutionReceipt): Promise<boolean>;
  ingestHarnessEvent(e: { path: string; event: unknown; producerBlake3: string }): Promise<void>;
  awaitHumanApproval(): Promise<void>;
};

// see: cans/01-architecture.md#VirtIO-Device-Complex
type RepoPolicyCtx = {
  policy: PolicyHandle;
};

const repoPolicyPlugin = (
  opts: { maxDiffBytes: number; protectedBranches: string[] },
): KelirBuilder<"plugin", "unsealed", "repo-policy", RepoPolicyCtx, "ledger", unknown> =>
  createPlugin<"repo-policy", RepoPolicyCtx, typeof opts>("repo-policy", { opts })
    .requires("ledger")
    .fs((f) =>
      f
        .onFsWrite((path, buffer, _pid, ctx) => {
          if (path.endsWith(".diff") && buffer.byteLength > opts.maxDiffBytes) {
            ctx.policy.recordViolation();
            return FilterAction.Drop("ENOSPC", "diff exceeds policy ceiling");
          }
          // in-place redaction: the bridge copies the mutated buffer back to the guest
          const redacted = buffer.toString("utf8")
            .replace(/sk_live_[0-9a-zA-Z]{24}/g, (m) => "*".repeat(m.length));
          buffer.set(Buffer.from(redacted, "utf8"), 0);
          return FilterAction.Forward;
        })
        .onFsUnlink((path: string) => {
          // Prevent rogue agent deletion of validation fixtures or manifests
          if (path.endsWith("package.json") || path.includes("/tests/")) {
            const filename = path.split("/").pop();
            return Verdict.Patch(PatchPayload.FsPathRedirect({
              targetPath: `/workspace/.agentsh_trash/${filename}`,
            }));
          }
          return Verdict.Pass;
        }),
    );

const paramsModel = t.Object({
  action:  t.Literal("fix-stripe-webhook"),
  repo:    t.String(),
  issueId: t.String(),
  env:     t.Union([t.Literal("staging"), t.Literal("production")]),
  force:   t.Optional(t.Boolean()),
});
type Params = Infer<typeof paramsModel>;

const buildRuntime = () =>
  kelir({
    vcpus: 4, memoryMb: 4096, driver: "auto",
    kernel: `./fixtures/vmlinux-6.6-${process.arch}-kelir`,
    rootfsErofs: "./fixtures/rootfs-swebench.erofs",
    tscLocked: true, enableWatchdog: true,
  })
    // 1. Plugins mount first; TPlugins accumulates ledger, chaos, policy
    .use(ledger({
      storePath: ".kelir/audit.merkle.db",
      merkleChain: true,
      otel: { propagateW3cTraceparent: true, serviceName: "kelir-swe-agent" },
      receipts: { includeDirtyBytes: true, includeBinaryBlake3: true, includeSyscallCount: true },
    }))
    .use(chaos({
      network: [{ target: "api.stripe.com", latencyMs: 150, jitterMs: 30, dropRate: 0.02 }],
      storage: [{ targetGlob: "/workspace/build/cache/*", simulateErrno: "enospc", probability: 0.05 }],
    }))
    .use(repoPolicyPlugin({ maxDiffBytes: 64 * 1024, protectedBranches: ["main", "master", "release/*"] }))
    .use(tokenShield({ maxBytesTotal: 128 * 1024, maxLinesTotal: 500, clampStrategy: "head-and-tail", notice: "⚠️ [SHIELD]" }))
    .use(terminalGuard({ prompts: [{ pattern: "(?i)Overwrite production schema\\? \\[y/N\\]", timeoutSeconds: 3,
                                     verifyForeground: true, action: [...Buffer.from("y\n")] }] }))
    .use(redaction({ maskRead: { engine: "fast-regex", carryoverBytes: 1024,
      rules: [{ glob: "**/secrets/**", patterns: [["sk_live_[0-9a-zA-Z]{24}", "[STRIPE_VAULT_KEY]"],
                                                   ["AKIA[0-9A-Z]{16}", "[AWS_IAM_REDACTED]"]] }] } }))
    .use(vault({
      credentials: [{
        name: "STRIPE_SECRET",
        placeholder: "vault://stripe_key",
        kind: "bearer-token",
        audience: "api.stripe.com"
      }]
    }))

    // 2. Planes and hooks mount next; c.cap("ledger") is statically inferred without manual generics
    .terminal({
      cols: 120, rows: 40,
      streamEngine: "StatelessRegex",
      sanitizer: { dropOsc52: true, stripTitle: true },
    })

    .storage({
      idmap: { guestUid: 0, hostUid: process.getuid!() },
      sharedCaches: [
        { hostPath: "~/.npm", guestMount: "/root/.npm", readOnly: true },
        { hostPath: "~/.cache/pip", guestMount: "/root/.cache/pip", readOnly: true },
        { hostPath: "~/.cargo/registry", guestMount: "/usr/local/cargo/registry", readOnly: true }
      ],
    })

    .wire({
      sinkhole: { rules: [{ host: "api.stripe.com", allow: true }, { host: "github.com", allow: true },
                            { host: "*.internal.net", allow: true }, { host: "*", allow: false }] },
      sshAgentBridge: true,
      l4Proxies: [{ listenPort: 5432, destination: "pg.internal.net:5432", injectMtls: { certPath: "/etc/ssl/client.crt", keyPath: "/etc/ssl/client.key" } }],
      tapFilter: "AllowUdpProxied",
    })

    // see: cans/03-sdk-architecture.md#Params-typed-views
    .params<Params>((p) => p.model(paramsModel))
    // see: cans/10-attestation.md#Execution-receipts
    .trap("*", {
      pre: async (c) => {
        await c.checkpoint.create(`pre:${c.trap.pid}`);
        const intent = await oracle.analyzeIntent({
          binary: c.trap.binaryPath,
          binaryBlake3: c.trap.binaryBlake3,
          argv: c.trap.argv, lineage: c.trap.ppid,
          cwd: c.trap.cwd,
        });
        if (!intent.isHypothesisValid)
          return c.drop("EPERM", `Verification Reject: ${intent.falsificationReason}`);
        return c.pass();
      },
      post: async (c) => {
        const pre = c.step.last();
        const receipt: ExecutionReceipt = await c.receipt();
        if (receipt.binaryBlake3 !== c.trap.binaryBlake3) {
          await c.vm.rollback(pre);
          c.terminal.notice("⚡ [KELIR]: post-exec binary hash mismatch — REVERTED\n");
          return PostExecAction.RollbackAndHalt;
        }
        if (c.trap.binaryPath.endsWith("/npm") && receipt.egressBytes > 0 &&
            c.trap.cwd.includes("/checkout-service")) {
          await c.vm.rollback(pre);
          c.terminal.notice(`⚡ [KELIR]: hermetic test emitted ${receipt.egressBytes}B — REVERTED\n`);
          return PostExecAction.RollbackAndHalt;
        }
        if (!(await oracle.verifyPhysicalReceipt(receipt))) {
          await c.vm.rollback(pre);
          c.terminal.notice("⚡ [KELIR]: post-condition violated — REVERTED\n");
          return PostExecAction.RollbackAndHalt;
        }
        return PostExecAction.Proceed;
      },
    })
    .trap(TrapFilter.binary("npm").withArg("test").withArg("--").withArg("--runInBand"), async (c) => {
      await c.fs.warmCache(["node_modules", ".next/cache"]);
      // see: cans/02-plugin-model.md#Patch-payload-is-registered-per-ring:-ExecveMutation,-FsInode,
      return c.patch({ binary: c.trap.binaryPath, argv: c.trap.argv,
                       env: { OTEL_TRACE_ID: oracle.currentTraceId(), CI: "1" } });
    })
    .trap(TrapFilter.binary("git").withArg("push"), async (c) => {
      // see: cans/03-sdk-architecture.md#Service-lookup-across-languages
      const repoPolicy = c.cap<RepoPolicyCtx>("repo-policy");
      if (repoPolicy && repoPolicy.policy.isProtected()) {
        repoPolicy.policy.recordViolation();
        return c.drop("EPERM", `EPOLICY: '${repoPolicy.policy.currentBranch}' protected.`);
      }
      if (c.trap.argv.includes("--force"))
        return c.patch({ binary: "/bin/git",
                         argv: c.trap.argv.filter((a) => a !== "--force"),
                         env: { KELIR_DISARMED: "1" } });
      return c.patch({
        binary: "/bin/git",
        argv: [...c.trap.argv, "HEAD:refs/heads/agent/demo-branch"],
        env:  { KELIR_REWRITTEN: "1" },
      });
    })
    // see: cans/10-attestation.md#Execution-receipts
    .trap(TrapFilter.binary("git").withArg("commit"), {
      pre: async (c) => {
        // see: cans/03-sdk-architecture.md#Service-lookup-across-languages
        const repoPolicy = c.cap<RepoPolicyCtx>("repo-policy");
        return repoPolicy && repoPolicy.policy.violations > 0
          ? c.drop("EPERM", `EPOLICY: ${repoPolicy.policy.violations} prior violation(s).`)
          : c.pass();
      },
      post: async (c) => {
        const receipt = await c.receipt();
        if (receipt.dirtyBytes === 0)
          throw new Error("Verification anomaly: zero-byte delta commit attempted.");
        // see: cans/10-attestation.md#Typed-extensions
        const vfsExt = receipt.fsVfs();
        console.log(vfsExt
          ? `[kelir] commit fs:vfs extension present, served_inodes=${vfsExt.servedInodes} roots=${vfsExt.roots.length}`
          : "[kelir] commit fs:vfs extension absent");
        return PostExecAction.Proceed;
      },
    })

    .lifecycle((l) =>
      l
        .onVmInit(async (driver) => driver.setWatchdog({ timeoutMs: 5 * 60_000, action: "reboot" }))
        .onVmReady(async (ctx) => { await ctx.execInGuest(["npm", "ci"]); })
        .onVmShutdown(async (ctx) => { await ctx.cap<LedgerHandle>("ledger")?.flush(); }),
    )
    // see: cans/08-lifecycle.md#Break-glass
    .deny((d) =>
      d.onTrapDeny(async (e, _verdict, s) => {
        if (await s.cap<LedgerHandle>("ledger")!.countDenials(e.ppid) > 5) {
          await s.vm.suspend("./.kelir/quarantine.snap");
          await s.console();
        }
      }),
    );

async function main() {
  const session = await buildRuntime().seal().boot();
  console.log(`[kelir] driver=${session.driverName} boot=${session.bootMs}ms`);

  // see: cans/07-wire-plane.md#Port-forwarding
  await session.wire.forwardPort(3000, 8080);
  const baseSnap = await session.checkpoint.create("clean_base");
  let winningDelta: CausalDelta | null = null;

  try {
    const testRun = await session.spawn("npm", ["test", "--", "--runInBand"], {
      cwd: "/workspace/checkout-service",
      params: { action: "fix-stripe-webhook", repo: "checkout-service",
                issueId: "ISSUE-8412", env: "staging" },
    });
    if (testRun.exitCode !== 0) { await session.vm.rollback(baseSnap); await session.console(); return; }

    const branches = await Promise.all(
      ["A", "B", "C", "D"].map(async (id) => {
        const child = await session.fork();
        // see: cans/08-lifecycle.md#Clock-skew
        await child.vm.clock.skew({ days: 24 * (id.charCodeAt(0) - 64) });
        const run = await child.spawn("npm", ["run", `verify-hypothesis-${id}`],
                                       { cwd: "/workspace/checkout-service" });
        // see: cans/03-sdk-architecture.md#Service-lookup-across-languages
        const delta = await child.cap<LedgerHandle>("ledger")!.getCausalDeltaFromParent();
        await child.terminate();
        return { id, run, delta };
      })
    );
    const winner = branches.find((b) => b.run.exitCode === 0);
    if (!winner) { await session.vm.rollback(baseSnap); return; }
    winningDelta = winner.delta;

    await session.suspend(SuspendReason.external("human-authorization-hold"));
    await oracle.awaitHumanApproval();
    await session.resume();

    // see: cans/05-storage-plane.md#Atomic-commit-reconciler
    const stats = await session.storage.reconcile({
      cwd: "/workspace/checkout-service",
      atomicStaging: true, fsync: true,
    });
    console.log(`[kelir] broker commit ${stats.filesChanged}f, ${stats.tombstonesResolved} deletions`);

    await session.spawn("git",
      ["commit", "-am", `fix(stripe): dedupe webhook body reads (${winner.id})`],
      { cwd: "/workspace/checkout-service" });

    const final = await session.cap<LedgerHandle>("ledger")!.finalize({ includeDelta: winningDelta!, includeMerkleRoot: true });
    console.log("[kelir] merkle root:", final.merkleRoot);
    console.log("[kelir] signed JWS: ", final.signedJws.slice(0, 80) + "…");
  } finally {
    await session.suspend(SuspendReason.external("human-authorization-hold"));
    await session.terminate();
  }
}

main().catch(console.error);
