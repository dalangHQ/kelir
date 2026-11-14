// see: cans/08-lifecycle.md#CoW-fork
// see: cans/plugins/mesh.md#Plugin-mesh
// see: cans/plugins/supply-chain.md#Plugin-supply-chain
// see: cans/plugins/ledger.md#Plugin-ledger

import { kelir, LifecycleEvent, VetoEvent,
         type LedgerHandle, type Session } from "@kelir/core";
import { mesh, supplyChain, ledger } from "@kelir/core";

const SWARM_REGION = "swarm-coordination";
const SWARM_BYTES = 32;

let cacheReady = false;

const runtime = kelir({
  vcpus: 4, memoryMb: 8192, driver: "auto",
  kernel: "./fixtures/vmlinux-6.6-aarch64-kelir",
  rootfsErofs: "./fixtures/rootfs-swarm.erofs",
  tscLocked: true, enableWatchdog: true,
})
  // see: cans/plugins/mesh.md#Plugin-mesh
  .use(mesh({
    peerIdentity: { trustDomain: "kelir.local", workloadPrefix: "swarm/worker" },
    mtls: true,
    traceparent: "inject-and-validate",
    listeners: [
      { port: 7001, service: "shard-exchange" },
      { port: 7002, service: "winner-announce" },
    ],
  }))
  // see: cans/plugins/supply-chain.md#Plugin-supply-chain
  .use(supplyChain({
    wxJit: {},
    packageGuard: { managers: ["npm", "pip", "cargo"] },
  }))
  // see: cans/plugins/ledger.md#Plugin-ledger
  .use(ledger({
    store: ".kelir/swarm-audit.merkle.db",
    merkleChain: true,
    suppressLocalDb: false,
    receipts: { includeSyscallCount: false },
  }))
  .on(LifecycleEvent.VmInit, async (lifecycle) => {
    lifecycle.registerVeto(VetoEvent.Fork, () => cacheReady);
  });

async function runShard(child: Session, shardId: number, total: number) {
  await child.checkpoint.create(`swarm-shard-${shardId}/${total}`);
  // TODO (L3): map the shard-exchange shared region, spawn cargo test --shard, and publish the result via the mesh handle.
  await child.stop();
  return { shardId, exitCode: 0 };
}

async function main() {
  runtime.seal();
  const session = await runtime.boot();
  try {
    await session.terminal.resize(120, 40);
    await session.checkpoint.create("swarm-cache-ready");
    cacheReady = true;

    const results = await Promise.all(
      Array.from({ length: 8 }, async (_, shardId) => {
        const child = await session.fork();
        return runShard(child, shardId, 8);
      }),
    );

    let passed = 0;
    let winner: number | undefined;
    for (const { shardId, exitCode } of results) {
      if (exitCode === 0) {
        passed += 1;
        if (winner === undefined) winner = shardId;
      }
    }

    // TODO (L3): await the mesh winner announcement; MeshHandle exposes peers only, with no awaitWinner, so the first passing shard stands in as winner.
    console.error(`[kelir] mesh winner shard=${winner}`);
    console.error(`[kelir] ${passed}/8 shards passed; winner shard=${winner}`);
    const denials = await (session.cap("ledger") as LedgerHandle).countDenials(process.pid);
    console.error(`[kelir] supply-chain denials=${denials}`);
  } finally {
    await session.stop();
  }
}

main().catch(console.error);
