// see: cans/08-lifecycle.md#CoW-fork
// see: cans/plugins/mesh.md#Plugin-mesh
// see: cans/plugins/supply-chain.md#Plugin-supply-chain
// see: cans/plugins/ledger.md#Plugin-ledger

import { kelir, LifecycleEvent, VetoEvent,
         type LedgerHandle, type MeshHandle, type Session } from "@kelir/core";
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
  const _shm = await child.shm(SWARM_REGION);
  const run = await child.spawn("cargo", ["test", "--", `--shard=${shardId}`]);
  if (run.exitCode === 0) {
    const meshHandle = child.cap("mesh") as MeshHandle;
    if (meshHandle) await meshHandle.broadcastWinner(shardId);
  }
  await child.stop();
  return { shardId, exitCode: run.exitCode };
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

    const meshHandle = session.cap("mesh") as MeshHandle;
    const meshWinner = await meshHandle.awaitWinner(2000);
    console.error(`[kelir] mesh winner shard=${meshWinner}`);
    console.error(`[kelir] ${passed}/8 shards passed; winner shard=${meshWinner}`);
    const denials = await (session.cap("ledger") as LedgerHandle).countDenials(process.pid);
    console.error(`[kelir] supply-chain denials=${denials}`);
  } finally {
    await session.stop();
  }
}

main().catch(console.error);
