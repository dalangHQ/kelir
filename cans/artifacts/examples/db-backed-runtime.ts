// see: cans/plugins/db-proxy.md#Plugin-db-proxy
// see: cans/plugins/ledger.md#Plugin-ledger
// see: cans/plugins/transaction.md#Plugin-transaction

import { kelir, t, TrapFilter, PostExecAction,
         type ExecutionReceipt } from "@kelir/core";
import { dbProxy, ledger, transaction } from "@kelir/core";

declare const env: {
  readonly KELIR_PG_HOST?: string;
  readonly KELIR_PG_SECRET?: string;
};

declare const readSnapshot: (path: string) => Promise<Uint8Array>;
declare const writeSnapshot: (path: string, data: Uint8Array) => Promise<void>;

const LOCAL_SNAPSHOT = "./fixtures/migrate-snapshot.erofs";

function buildRuntime(pgHost: string, _pgSecret: string) {
  return kelir({
    vcpus: 2, memoryMb: 2048, driver: "auto",
    kernel: `./fixtures/vmlinux-6.6-${process.arch}-kelir`,
    rootfsErofs: "./fixtures/rootfs-migrate.erofs",
    tscLocked: true, enableWatchdog: true,
  })

    // see: cans/plugins/db-proxy.md#L7-pgwire-rewrite-and-2PC-commit
    .use(dbProxy({
      postgres: {
        listenPort: 5432,
        upstream: `${pgHost}:5432`,
        rewriteRules: [["public.", "migrate_schema."]],
      },
      // see: cans/plugins/db-proxy.md#Ephemeral-credentials-injected-at-host-connection-boundary
      credentials: {
        placeholder: "vault://pg/app_migrate",
        ttlSeconds: 300,
      },
      sqlite: {
        guestSocket: "/run/kelir/db.sock",
        hostPath: "/var/lib/app/migrations.sqlite",
      },
    }))

    // see: cans/plugins/ledger.md#Merkle-WAL
    .use(ledger({
      store: "sqlite://.kelir/migrate-audit.merkle.db",
      merkleChain: true,
      otel: undefined,
      receipts: { includeSyscallCount: false },
    }))

    // see: cans/plugins/transaction.md#Two-phase-commit-demonstration
    .use(transaction({
      timeoutMs: 30000,
      autoRollbackOnTaint: true,
      flushFuseBeforeCommit: true,
    }))
    // see: cans/plugins/db-proxy.md#Plugin-db-proxy
    .params(t.Object({
      action: t.Literal("migrate"),
      schemaName: t.String(),
      targetVersion: t.Number(),
      dryRun: t.Boolean(),
    }))
    .trap(TrapFilter.binary("migrate"), {
      pre: async (c) => {
        // TODO (L3): drop unless the db-proxy rewrite target is "migrate_schema."; the db-proxy plugin exposes no cap handle in @kelir/core, so the check cannot read the live target yet.
        return c.pass();
      },
      post: async (c) => {
        const receipt: ExecutionReceipt = await c.receipt();
        if (!receipt.extensions.some((e) => e.namespace === "net:l7:sql"))
          return PostExecAction.RollbackAndHalt;
        return PostExecAction.Proceed;
      },
    })
    // see: cans/plugins/transaction.md#Plugin-transaction
    .onPreCommit(async (ctx) => {
      await ctx.cap?.ledger?.flush();
    })
    .onPostCommit(async (_ctx, committed) => {
      if (committed) {
        console.log("[kelir] commit: per-entry leaf commit, staged snapshot "
                    + "is next cold-start fixture");
      } else {
        console.log("[kelir] commit aborted: staging unlinked, broker upper "
                    + "and tombstones dropped, last good snapshot kept");
      }
    });
}

async function main() {
  const pgHost = env.KELIR_PG_HOST;
  const pgSecret = env.KELIR_PG_SECRET;
  if (!pgHost || !pgSecret) throw new Error("KELIR_PG_HOST and KELIR_PG_SECRET must be set");

  const runtime = buildRuntime(pgHost, pgSecret);
  runtime.seal();
  const session = await runtime.boot();
  try {
    // see: cans/09-snapshots.md#Snapshots
    await session.vm.importSnapshot(await readSnapshot(LOCAL_SNAPSHOT));

    // TODO (L3): open the transaction scope "migrate-42" over /workspace/migrations + migrate_schema, savepoint before the migrate, and roll back to it on failure; @kelir/core exposes no transaction scope API yet.
    const run = await session.spawn("python", ["-m", "app.migrate", "--schema", "migrate"], {
      cwd: "/workspace/migrations",
      params: { action: "migrate", schemaName: "migrate", targetVersion: 42, dryRun: false },
    });
    if (run.exitCode !== 0) throw new Error(`migration failed: ${run.exitCode}`);

    const out = await session.vm.exportSnapshot();
    await writeSnapshot(LOCAL_SNAPSHOT, out);

    // see: cans/10-attestation.md#Execution-receipts
    const final = await session.ledger.finalize({
      includeMerkleRoot: true,
      includeDelta: await session.ledger.getCausalDeltaFromParent(),
    });
    console.log(`[kelir] merkle=${final.merkleRoot} jws=${final.signedJws.slice(0, 64)}…`);
  } finally {
    await session.terminate();
  }
}

main().catch(console.error);
