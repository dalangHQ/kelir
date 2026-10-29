// User-initiated forwardPort() binds a host-side listener on the named
// port. This is the host-initiated path; the guest-initiated PortBound
// tunnel in 07-lifecycle.md#Port-forwarding creates no host listener.

// ============================================================================
// autonomous-swe-agent.ts
// ============================================================================

import { kelir, createPlugin, t,
         type Session, type SnapshotDescriptor, type ExecutionReceipt,
         type PostExecAction, type CausalDelta } from "@kelir/core";
import { chaos }  from "@kelir/plugin-chaos";
import { ledger } from "@kelir/plugin-ledger";
import crypto from "node:crypto";

// Oracle: post-condition verification engine.
declare const oracle: {
  currentTraceId(): string;
  analyzeIntent(trap: { binary: string; binaryBlake3: string; argv: string[];
                        lineage: number; cwd: string }): Promise<{ isHypothesisValid: boolean;
                                                          falsificationReason?: string }>;
  verifyPhysicalReceipt(r: ExecutionReceipt): Promise<boolean>;
  ingestHarnessEvent(e: { path: string; event: unknown; producerBlake3: string }): Promise<void>;
  awaitHumanApproval(): Promise<void>;
};

// ── Plugin: repo policy (contributes `c.policy`) ──────────────────────────
// Plugin: repo-policy; rings: virtio-vsock, virtio-fs
// Its push gate lives in the runtime "git push *" trap below; its
// maxDiffBytes ceiling lives in the vfs diff-ceiling snoop below.
const repoPolicyPlugin = (opts: { maxDiffBytes: number; protectedBranches: string[] }) =>
  createPlugin("repo-policy")
    .state("violations", 0)
    .state("branch", "" as string)
    .decorate("policy", (store) => ({
      violations: store.violations,
      currentBranch: store.branch,
      isProtected: () => opts.protectedBranches.includes(store.branch),
    }));

// ============================================================================
// SINGLE LINEAR CHAIN — order defines context keys; nothing mutates after boot
// ============================================================================

const runtime = kelir({
  vcpus: 4, memoryMb: 4096, driver: "auto",
  kernel: `./fixtures/vmlinux-6.6-${process.arch}-kelir`,
  rootfsErofs: "./fixtures/rootfs-swebench.erofs",
  tscLocked: true, enableWatchdog: true,
})

  // → native terminal plane
  .terminal({
    cols: 120, rows: 40,
    dropOsc52: true, stripTitle: true,
    tokenShield: { maxBytesTotal: 128 * 1024, maxLinesTotal: 500,
                   clampStrategy: "head-and-tail",
                   onClamp: (s) => `\n⚠️ [SHIELD] -${s.droppedBytes}B / -${s.droppedLines}L\n` },
    prompts: [{ match: /Overwrite production schema\? \[y\/N\]/i,
                timeoutMs: 3000, verifyForeground: true,
                action: async (_p, stream) => { stream.write("y\n"); } }],
    // Non-default knob (04-terminal-plane.md#Terminal-pipeline):
    // StatelessRegex instead of the StreamingAutomata default.
    // The filters above are unchanged; only the match engine is.
    streamEngine: "StatelessRegex",
  }))

  // → native storage plane
  .storage({
    idmap: { guestUid: 0, hostUid: process.getuid!() },
    sharedCaches: ["~/.npm", "~/.cache/pip", "~/.cargo/registry"],
    lockBroker: true,
    syntheticFiles: [
      { path: "/workspace/.env.local",
        content: "DATABASE_URL=postgres://app:vault@127.0.0.1:5432/prod\n" },
      { path: "/workspace/identity.jwt",
        // Lazy per-read thunk, like Py SyntheticFile.dynamic / RS
        // SyntheticFile::dynamic — not frozen at builder time.
        content: () => `Bearer ${crypto.randomBytes(32).toString("hex")}`, mode: 0o400 },
    ],
    maskRead: [
      { glob: "**/secrets/**",
        pattern: /sk_live_[0-9a-zA-Z]{24}/g, replace: "[STRIPE_VAULT_KEY]" },
      { glob: "**/secrets/**",
        pattern: /AKIA[0-9A-Z]{16}/g,       replace: "[AWS_IAM_REDACTED]" },
    ],
    // NOTE: onWrite does not capture `runtime` or `session`. It receives
    // the typed handler context `c`, whose `c.ledger` is bound by the
    // `ledger()` plugin below; snoop onWrite only fires at guest write
    // time, which is post-boot.
    snoops: [
      { path: "~/.claude/sessions/*.json", sync: true,
        onWrite: async (path, raw) => {
          const evt = JSON.parse(raw.toString());
          if (evt.falsifiedResult) throw new Error("EPERM: harness hallucination detected.");
          await oracle.ingestHarnessEvent({
            path, event: evt, producerBlake3: "verified-process-identity"
          });
        } },
      // Diff ceiling (repo-policy maxDiffBytes): oversize diffs are
      // rejected at the ring with ENOSPC instead of landing on disk.
      { path: "**/*.diff", sync: true,
        onWrite: async (_path, raw) => {
          if (raw.byteLength > 64 * 1024)
            throw new Error("ENOSPC: diff exceeds policy ceiling");
        } },
    ],
  }))

  // → native wire plane
  .wire({
    sinkhole: { dnsExcept: ["api.stripe.com", "github.com", "*.internal.net"] },
    sshAgentBridge: true,
    vaults: [{ name: "STRIPE_SECRET",
               placeholder: "vault://stripe_key",
               resolve: () => process.env.REAL_HOST_STRIPE_KEY!,
               scopeUrls: ["https://api.stripe.com/v1/*"],
               scrubResponseHeaders: ["set-cookie", "request-id"] }],
    l4Proxies: [{ listenPort: 5432, destination: "pg.internal.net:5432",
                  injectMtls: { certPath: "/etc/ssl/client.crt",
                                keyPath:  "/etc/ssl/client.key" } }],
    // Non-default knob (06-wire-plane.md): AllowUdpProxied instead of the
    // StrictTcpOnly default. The sinkhole allowlist above is unchanged;
    // only the UDP path moves to proxied.
    tapFilter: "AllowUdpProxied",
  }))

  // → c.chaos
  .use(chaos({
    network: [{ target: "api.stripe.com",
                latency: { baseMs: 150, jitterMs: 30 }, dropRate: 0.02 },
              { target: "api.stripe.com",
                latency: { baseMs: 150, jitterMs: 30 }, dropRate: 0.02,
                direction: "inbound" },
              { target: "api.stripe.com",
                latency: { baseMs: 150, jitterMs: 30 }, dropRate: 0.02,
                direction: "both" }],
    fs: [{ targetPath: "/workspace/build/cache/*",
           simulateError: "ENOSPC", errorProbability: 0.05 }],
  }))

  // → c.ledger
  .use(ledger({
    // audit DB lives on the host, immune to guest rollback/fork
    store: "sqlite://.kelir/audit.merkle.db",
    merkleChain: true,
    otel: { traceParent: true, serviceName: "kelir-swe-agent" },
    receipts: { includeDirtyBytes: true, includeBinaryBlake3: true, includeSyscallCount: true },
  }))

  // → c.policy
  .use(repoPolicyPlugin({ maxDiffBytes: 64 * 1024,
                          protectedBranches: ["main", "master", "release/*"] }))

  // → typed params + trap pipeline
  .params(t.Object({
    action:  t.Literal("fix-stripe-webhook"),
    repo:    t.String(),
    issueId: t.String(),
    env:     t.Union([t.Literal("staging"), t.Literal("production")]),
    force:   t.Optional(t.Boolean()),
  }))
  .trap("*", {
    pre: async (c) => {
      await c.vm.checkpoint(`pre:${c.trap.pid}`);
      const intent = await oracle.analyzeIntent({
        binary: c.trap.binary_path,
        binaryBlake3: c.trap.binary_blake3,
        argv: c.trap.argv, lineage: c.trap.ppid,
        cwd: c.trap.cwd,
      });
      if (!intent.isHypothesisValid)
        return c.deny(1, `Verification Reject: ${intent.falsificationReason}`);
      return c.resume();
    },
    post: async (c) => {
      const pre = c.step.last();
      const receipt: ExecutionReceipt = await c.receipt();
      if (receipt.binary_blake3 !== c.trap.binary_blake3) {
        await c.vm.rollback(pre);
        session.terminal.notice("⚡ [KELIR]: post-exec binary hash mismatch — REVERTED\n");
        return PostExecAction.RollbackAndHalt;
      }
      if (c.trap.binary_path.endsWith("/npm") && receipt.egress_bytes > 0 &&
          c.trap.cwd.includes("/checkout-service")) {
        await c.vm.rollback(pre);
        session.terminal.notice(`⚡ [KELIR]: hermetic test emitted ${receipt.egress_bytes}B — REVERTED\n`);
        return PostExecAction.RollbackAndHalt;
      }
      if (!(await oracle.verifyPhysicalReceipt(receipt))) {
        await c.vm.rollback(pre);
        session.terminal.notice("⚡ [KELIR]: post-condition violated — REVERTED\n");
        return PostExecAction.RollbackAndHalt;
      }
      return PostExecAction.Proceed;
    },
  })
  .trap(TrapFilter.binary("npm").withArg("test").withArg("--").withArg("--runInBand"), async (c) => {
    await c.fs.warmCache(["node_modules", ".next/cache"]);
    // Env injection via Rewrite, not env.set + resume (09 Env-injection:
    // env.set + resume returns EINVALIDDECISION). Same binary/argv.
    return c.rewrite({ binary: c.trap.binary_path, argv: c.trap.argv,
                       env: { OTEL_TRACE_ID: oracle.currentTraceId(), CI: "1" } });
  })
  .trap(TrapFilter.binary("git").withArg("push"), async (c) => {
    // Merged repo-policy push gate + runtime push gate.
    if (c.policy.isProtected()) {
      c.store.violations += 1;
      return c.deny(1, `EPOLICY: '${c.policy.currentBranch}' protected.`);
    }
    if (c.trap.argv.includes("--force"))
      return c.rewrite({ binary: "/bin/git",
                         argv: c.trap.argv.filter((a) => a !== "--force"),
                         env: { KELIR_DISARMED: "1" } });
    return c.rewrite({
      binary: "/bin/git",
      argv: ["git", "push", "origin", `HEAD:refs/heads/agent/${crypto.randomUUID().slice(0, 8)}`],
      env:  { KELIR_REWRITTEN: "1" },
    });
  })
  .trap(TrapFilter.binary("git").withArg("commit"), {
    pre: async (c) =>
      c.policy.violations > 0
        ? c.deny(1, `EPOLICY: ${c.policy.violations} prior violation(s).`)
        : c.resume(),
    post: async (c) => {
      const receipt = await c.receipt();
      // dirtyBytes is always populated; zero is a real measurement
      if (receipt.dirtyBytes === 0)
        throw new Error("Verification anomaly: zero-byte delta commit attempted.");
      // Workspace-scoped observation (08-epistemic.md#Execution-receipts):
      // dirtyBytes counts all hypervisor-dirtied bytes; the workspace
      // counter now arrives as a native storage plane TelemetryExtension
      // (namespace "fs:vfs"), whose payload is plugin-defined and
      // opaque to core. The gate above still keys on dirtyBytes;
      // this logs only the extension's presence and payload length,
      // never a decoded count.
      const vfsExt = receipt.extensions?.find((e) => e.namespace === "fs:vfs");
      console.log(vfsExt
        ? `[kelir] commit fs:vfs extension present, payload_len=${vfsExt.payload.length}`
        : "[kelir] commit fs:vfs extension absent");
      return PostExecAction.Proceed;
    },
  })

  // ── LIFECYCLE: vm:init receives the driver; the rest receive `session`
  //    typed against the full accumulated context. No handler references
  //    a pre-boot variable. ────
  .on(LifecycleEvent.VmInit,     async (driver) => driver.setWatchdog({ timeoutMs: 5 * 60_000, action: "reboot" }))
  .on(LifecycleEvent.VmReady,    async (s) => { await s.execInGuest("npm", ["ci", "--prefer-offline"]); })
  .on(LifecycleEvent.VmShutdown, async (s) => { await s.use(ledgerExtension).flush(); })
  .on(LifecycleEvent.TrapDeny,   async (e, s) => {
    if (await s.use(ledgerExtension).countDenials(e.ppid) > 5) {
      await s.vm.suspend("./.kelir/quarantine.snap");
      await s.breakGlass();
    }
  });

// ============================================================================
// BOOT — the chain terminates here. Everything after uses `session`, which
// re-exposes the SAME typed surface (s.vm, s.policy, s.step…);
// domain APIs arrive via session.use(extension).
// ============================================================================

async function main() {
  const session = await runtime.boot();
  console.log(`[kelir] driver=${session.driverName} boot=${session.bootMs}ms`);

  await session.wire.forwardPort({ host: 8080, guest: 3000 });
  const baseSnap = await session.vm.checkpoint("clean_base");
  let winningDelta: CausalDelta | null = null;

  try {
    const testRun = await session.spawn("npm", ["test", "--", "--runInBand"], {
      cwd: "/workspace/checkout-service",
      // params are typed by .params and cross into the guest as the
      // KELIR_PARAMS=<json> environment variable
      params: { action: "fix-stripe-webhook", repo: "checkout-service",
                issueId: "ISSUE-8412", env: "staging" },
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

    // Sync the guest overlay to host storage, then commit via plain guest git.
    const stats = await session.storage.reconcile({
      cwd: "/workspace/checkout-service",
      atomicStaging: true, fsync: true,
    });
    console.log(`[kelir] reconciled ${stats.filesChanged}f, ${stats.whiteoutsResolved} deletions`);

    await session.spawn("git",
      ["commit", "-am", `fix(stripe): dedupe webhook body reads (${winner.id})`],
      { cwd: "/workspace/checkout-service" });

    const final = await session.use(ledgerExtension).finalize({ includeDelta: winningDelta!, includeMerkleRoot: true });
    console.log("[kelir] merkle root:", final.merkleRoot);
    console.log("[kelir] signed JWS: ", final.signedJws.slice(0, 80) + "…");
  } finally {
    await session.suspend("/tmp/eval.snap");
    await session.terminate();
  }
}

main().catch(console.error);

**The rule this file obeys:** *the builder is write-only until `boot()`; the session is read-only afterward; every plugin-contributed context key comes from exactly one `.use()` upstream of its first consumer (core provides `c.step` and `c.receipt()` directly); every state slot used across hooks is declared by a plugin via `.state()` + `.decorate()`.*
