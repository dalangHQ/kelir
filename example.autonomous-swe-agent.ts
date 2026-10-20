// ============================================================================
// autonomous-swe-agent.ts 
// ============================================================================

import { kelir, createPlugin, t,
         type Session, type SnapshotDescriptor, type ExecutionReceipt,
         type CausalDelta } from "@kelir/core";
import { pty }    from "@kelir/plugin-pty";
import { vfs }    from "@kelir/plugin-vfs";
import { wire }   from "@kelir/plugin-wire";
import { chaos }  from "@kelir/plugin-chaos";
import { ledger } from "@kelir/plugin-ledger";
import crypto from "node:crypto";

declare const oracle: {
  currentTraceId(): string;
  analyzeIntent(trap: { binary: string; binaryBlake3: number[]; argv: string[];
                        lineage: string[]; cwd: string; entropy: number;
                        loadedLibs: string[] }): Promise<{ isHypothesisValid: boolean;
                                                          falsificationReason?: string;
                                                          allowHighEntropy?: boolean }>;
  verifyPhysicalReceipt(r: ExecutionReceipt): Promise<boolean>;
  ingestHarnessEvent(e: { path: string; event: unknown; producerBlake3: string }): Promise<void>;
  awaitHumanApproval(): Promise<void>;
};

// ── Plugin: typed checkpoint slot (replaces the invented `c.scratch`) ──────
const stepLoopPlugin = () =>
  createPlugin("step-loop")
    .state("lastCheckpoint", null as SnapshotDescriptor | null)
    .decorate("step", (store) => ({
      mark: (cp: SnapshotDescriptor) => { store.lastCheckpoint = cp; },
      last: (): SnapshotDescriptor => {
        if (!store.lastCheckpoint) throw new Error("step-loop invariant: no checkpoint");
        return store.lastCheckpoint;
      },
    }));

// ── Plugin: repo policy (unchanged, still contributes exactly `c.policy`) ──
const repoPolicyPlugin = (opts: { maxDiffBytes: number; protectedBranches: string[] }) =>
  createPlugin("repo-policy")
    .state("violations", 0)
    .state("branch", "" as string)
    .decorate("policy", (store) => ({
      violations: store.violations,
      currentBranch: store.branch,
      isProtected: () => opts.protectedBranches.includes(store.branch),
    }))
    .exec((cmd) => cmd.before("git push *", async (c) =>
      c.policy.isProtected()
        ? (c.store.violations += 1, c.deny(1, `EPOLICY: '${c.policy.currentBranch}' protected.`))
        : c.rewrite({
            binary: "/bin/git",
            argv: ["git", "push", "origin", `HEAD:refs/heads/agent/${crypto.randomUUID().slice(0, 8)}`],
            env:  { KELIR_REWRITTEN: "1" },
            notice: "⚡ [KELIR]: push target rewritten to agent branch.\n",
          })
    ))
    .vfs((fs) => fs.onWrite(async (path, data, c) => {
      if (path.endsWith(".diff") && data.byteLength > opts.maxDiffBytes) {
        c.store.violations += 1;
        return { reject: "ENOSPC", reason: "diff exceeds policy ceiling" };
      }
      return { allow: true };
    }));

// ============================================================================
// SINGLE LINEAR CHAIN — order defines context keys; nothing mutates after boot
// ============================================================================

const runtime = kelir({
  vcpus: 4, memoryMb: 4096, driver: "auto",
  kernel: "./fixtures/vmlinux-6.6-arm64-kelir",
  rootfsErofs: "./fixtures/rootfs-swebench.erofs",
  tscLocked: true, enableWatchdog: true,
})

  // → c.term
  .use(pty((term) => term
    .windowSize({ cols: 120, rows: 40 })
    .dropOsc52().stripTitle()
    .tokenShield({ maxBytesTotal: 128 * 1024, maxLinesTotal: 500,
                   clampStrategy: "head-and-tail",
                   onClamp: (s) => `\n⚠️ [SHIELD] -${s.droppedBytes}B / -${s.droppedLines}L\n` })
    .onPrompt({ match: /Overwrite production schema\? \[y\/N\]/i,
                timeoutMs: 3000, verifyForeground: true,
                action: async (_p, stream) => { stream.write("y\n"); } })
  ))

  // → c.fs   (maskRead folded in; NO separate dlp() — that was break #1)
  .use(vfs((fs) => fs
    .idmap({ guestUid: 0, hostUid: process.getuid!() })
    .cacheSharedMounts(["~/.npm", "~/.cache/pip", "~/.cargo/registry"])
    .lockBroker({ enabled: true })
    .ghost("/workspace/.env.local",
           "DATABASE_URL=postgres://app:vault@127.0.0.1:5432/prod\n")
    .ghostDynamic("/workspace/identity.jwt", async () => ({
      content: `Bearer ${crypto.randomBytes(32).toString("hex")}`, mode: 0o400,
    }))
    .maskRead("**/secrets/**", [
      { pattern: /sk_live_[0-9a-zA-Z]{24}/g, replace: "[STRIPE_VAULT_KEY]" },
      { pattern: /AKIA[0-9A-Z]{16}/g,       replace: "[AWS_IAM_REDACTED]" },
    ])
    // NOTE: onWrite does NOT capture `runtime` or `session` (break #5).
    // It receives the typed handler context `c`, whose `c.ledger` is
    // bound by the `ledger()` plugin below. Order guarantees this is safe:
    // snoop.onWrite only fires at guest write time, which is post-boot.
    .snoop("~/.claude/sessions/*.json", {
      sync: true,
      onWrite: async (path, raw, c) => {
        const evt = JSON.parse(raw.toString());
        if (evt.falsifiedResult) throw new Error("EPERM: harness hallucination detected.");
        await oracle.ingestHarnessEvent({
          path, event: evt,
          producerBlake3: await c.ledger.blake3OfProducingProcess(path), // typed
        });
      },
    })
  ))

  // → c.wire
  .use(wire((w) => w
    .sinkholeDnsExcept(["api.stripe.com", "github.com", "*.internal.net"])
    .forwardSshAgent()
    .vault("STRIPE_SECRET", {
      placeholder: "vault://stripe_key",
      resolve: () => process.env.REAL_HOST_STRIPE_KEY!,
      scopeUrls: ["https://api.stripe.com/v1/*"],
      scrubResponseHeaders: ["set-cookie", "request-id"],
    })
    .proxyL4({ listenPort: 5432, destination: "pg.internal.net:5432",
               injectMtls: { certPath: "/etc/ssl/client.crt",
                             keyPath:  "/etc/ssl/client.key" } })
  ))

  // → c.chaos
  .use(chaos((ch) => ch
    .network({ target: "api.stripe.com",
               latency: { baseMs: 150, jitterMs: 30 }, dropRate: 0.02 })
    .fs({ targetPath: "/workspace/build/cache/*",
          simulateError: "ENOSPC", errorProbability: 0.05 })
  ))

  // → c.ledger
  .use(ledger({
    store: "sqlite:///workspace/audit.merkle.db",
    merkleChain: true,
    otel: { traceParent: true, serviceName: "kelir-swe-agent" },
    receipts: { includeDirtyPages: true, includeBinaryBlake3: true, includeSyscallCount: true },
  }))

  // → c.step   (typed checkpoint slot — replaces break #2)
  .use(stepLoopPlugin())

  // → c.policy
  .use(repoPolicyPlugin({ maxDiffBytes: 64 * 1024,
                          protectedBranches: ["main", "master", "release/*"] }))

  // → typed params (from guard) + handler pipeline
  .exec((cmd) => cmd
    .guard({ schema: t.Object({
      action:  t.Literal("fix-stripe-webhook"),
      repo:    t.String(),
      issueId: t.String(),
      env:     t.Union([t.Literal("staging"), t.Literal("production")]),
      force:   t.Optional(t.Boolean()),
    }) })
    .before("*", async (c) => {
      c.step.mark(await c.vm.checkpoint(`pre:${c.trap.pid}`));   // typed
      const intent = await oracle.analyzeIntent({
        binary: c.trap.binary_path,
        binaryBlake3: Array.from(c.trap.binary_blake3),
        argv: c.trap.argv, lineage: c.trap.ancestor_chain,
        cwd: c.trap.cwd, entropy: c.trap.entropy,
        loadedLibs: c.trap.loaded_libraries,
      });
      if (!intent.isHypothesisValid)
        return c.deny(1, `Epistemic Reject: ${intent.falsificationReason}`);
      if (c.trap.entropy > 5.2 && !intent.allowHighEntropy)
        return c.deny(126, "EENTROPY: high-entropy argv requires oracle clearance.");
      return c.resume();
    })
    .before("npm test -- --runInBand", async (c) => {
      c.env.set("OTEL_TRACE_ID", oracle.currentTraceId());
      c.env.set("CI", "1");
      await c.fs.warmCache(["node_modules", ".next/cache"]);
      return c.resume();
    })
    .before("git push *", async (c) => {
      if (c.trap.entropy > 5.2) return c.deny(1, "EPERM: high-entropy payload.");
      if (c.trap.argv.includes("--force"))
        return c.rewrite({ binary: "/bin/git",
                           argv: c.trap.argv.filter((a) => a !== "--force"),
                           env: { KELIR_DISARMED: "1" },
                           notice: "⚡ [KELIR NOTICE]: dropped '--force'.\n" });
      return c.resume();
    })
    .after("*", async (c) => {
      const pre = c.step.last();                                  // typed, no !
      const receipt: ExecutionReceipt = await c.ledger.lastReceipt(c.trap.pid);
      if (receipt.binary_blake3.join() !== Array.from(c.trap.binary_blake3).join()) {
        await c.vm.rollback(pre);
        c.term.notice("⚡ [KELIR]: post-exec binary hash mismatch — REVERTED\n");
        return;
      }
      if (c.trap.binary_path.endsWith("/npm") && receipt.egress_bytes > 0 &&
          c.trap.cwd.includes("/checkout-service")) {
        await c.vm.rollback(pre);
        c.term.notice(`⚡ [KELIR]: hermetic test emitted ${receipt.egress_bytes}B — REVERTED\n`);
        return;
      }
      if (!(await oracle.verifyPhysicalReceipt(receipt))) {
        await c.vm.rollback(pre);
        c.term.notice("⚡ [KELIR]: post-condition violated — REVERTED\n");
      }
    })
    .before("git commit -m *", async (c) =>
      c.policy.violations > 0
        ? c.deny(1, `EPOLICY: ${c.policy.violations} prior violation(s).`)
        : c.resume())
    .after("git commit *", async (c) => {
      if ((await c.getReceipt()).dirty_memory_pages === 0)
        throw new Error("Epistemic anomaly: zero-page delta commit attempted.");
    })
  )

  // ── LIFECYCLE: every handler receives `session` typed against the full
  //    accumulated context. No handler references a pre-boot variable. ────
  .on("vm:init",     async (_s, driver) => driver.watchdog({ timeoutMs: 5 * 60_000, action: "reboot" }))
  .on("vm:ready",    async (s) => { await s.execInGuest("npm", ["ci", "--prefer-offline"]); })
  .on("vm:shutdown", async (s) => { await s.ledger.flush(); })
  .on("trap:deny",   async (e, s) => {
    if (await s.ledger.countDenials(e.pid) > 5) {
      await s.vm.suspend("./.kelir/quarantine.snap");
      await s.breakGlass();
    }
  });

// ============================================================================
// BOOT — the chain terminates here. Everything after uses `session`, which
// re-exposes the SAME typed surface (s.vm, s.fs, s.ledger, s.policy, s.step…).
// ============================================================================

async function main() {
  const session = await runtime.boot();
  console.log(`[kelir] driver=${session.driverName} boot=${session.bootMs}ms`);

  await session.forwardPort({ host: 8080, guest: 3000 });
  const baseSnap = await session.vm.checkpoint("clean_base");
  let winningDelta: CausalDelta | null = null;

  try {
    const testRun = await session.spawn("npm", ["test", "--", "--runInBand"], {
      cwd: "/workspace/checkout-service",
      params: { action: "fix-stripe-webhook", repo: "checkout-service",
                issueId: "ISSUE-8412", env: "staging" },  // typed by the guard
    });
    if (testRun.exitCode !== 0) { await session.vm.rollback(baseSnap); await session.breakGlass(); return; }

    const branches = await Promise.all(
      ["A", "B", "C", "D"].map(async (id) => {
        const child = await session.vm.fork();                    // <15ms
        await child.vm.clock.skew({ days: 24 * (id.charCodeAt(0) - 64) });
        const run = await child.spawn("npm", ["run", `verify-hypothesis-${id}`],
                                       { cwd: "/workspace/checkout-service" });
        const delta = await child.getCausalDeltaFromParent();
        await child.terminate();
        return { id, run, delta };
      })
    );
    const winner = branches.find((b) => b.run.exitCode === 0);
    if (!winner) { await session.vm.rollback(baseSnap); return; }
    winningDelta = winner.delta;

    await session.suspend("/tmp/eval.snap");                      // → 0 MB RSS
    await oracle.awaitHumanApproval();
    await session.resume("/tmp/eval.snap");                       // <50ms

    const commit = await session.commit({
      cwd: "/workspace/checkout-service",
      message: `fix(stripe): dedupe webhook body reads (${winner.id})`,
      reconcileWhiteouts: true, atomicRename: true, fsync: true,
    });
    console.log(`[kelir] committed ${commit.filesChanged}f, ${commit.whiteoutsResolved} deletions`);

    const final = await session.ledger.finalize({ includeDelta: winningDelta!, includeMerkleRoot: true });
    console.log("[kelir] merkle root:", final.merkleRoot);
    console.log("[kelir] signed JWS: ", final.signedJws.slice(0, 80) + "…");
  } finally {
    await session.suspend("/tmp/eval.snap");
    await session.terminate();
  }
}

main().catch(console.error);
```

**The rule the repaired version now obeys:** *the builder is write-only until `boot()`; the session is read-only afterward; every context key is contributed by exactly one `.use()` upstream of its first consumer; every state slot used across hooks is declared by a plugin via `.state()` + `.decorate()`.*
