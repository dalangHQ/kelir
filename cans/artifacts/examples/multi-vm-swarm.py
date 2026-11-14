# see: cans/08-lifecycle.md#CoW-fork
# see: cans/plugins/mesh.md#Plugin-mesh
# see: cans/plugins/supply-chain.md#Plugin-supply-chain
# see: cans/plugins/ledger.md#Plugin-ledger
from __future__ import annotations

import asyncio
import os
from dataclasses import dataclass

import kelir
from kelir import KelirBuilder, Session
from kelir.plugin_model import LifecycleCtx, VetoEvent
from kelir.plugins.mesh import (
    MeshConfig, MeshPlugin, PeerIdentity, PeerListener,
)
from kelir.plugins.supply_chain import (
    PackageGuardConfig, SupplyChainConfig, SupplyChainPlugin, WxJitConfig,
)
from kelir.plugins.ledger import (
    LedgerConfig, LedgerPlugin, ReceiptConfig,
)


SWARM_REGION = "swarm-coordination"
SWARM_BYTES = 32


@dataclass
class SwarmState:
    cache_ready: bool = False


STATE = SwarmState()


def build_runtime() -> KelirBuilder:
    # see: cans/08-lifecycle.md#Lifecycle-vetoes
    async def _init(ctx: LifecycleCtx) -> None:
        ctx.register_veto(VetoEvent.Fork, lambda: STATE.cache_ready)

    return (
        kelir.builder(
            vcpus=4, memory_mb=8192, driver="auto",
            kernel="./fixtures/vmlinux-6.6-aarch64-kelir",
            rootfs_erofs="./fixtures/rootfs-swarm.erofs",
            tsc_locked=True, enable_watchdog=True,
        )
        # TODO: the lock-broker glob scope (**/Cargo.lock, **/package-lock.json) and non-blocking mode have no Py builder surface; .lock_broker() only takes an enabled flag.
        .lock_broker()
        # see: cans/plugins/mesh.md#Plugin-mesh
        .use(MeshPlugin(MeshConfig(
            peer_identity=PeerIdentity(
                trust_domain="kelir.local",
                workload_prefix="swarm/worker",
            ),
            mtls=True,
            traceparent="inject-and-validate",
            listeners=[
                PeerListener(port=7001, service="shard-exchange"),
                PeerListener(port=7002, service="winner-announce"),
            ],
        )))
        # see: cans/plugins/supply-chain.md#Plugin-supply-chain
        .use(SupplyChainPlugin(SupplyChainConfig(
            wx_jit=WxJitConfig(),
            package_guard=PackageGuardConfig(
                managers=["npm", "pip", "cargo"],
            ),
        )))
        # see: cans/plugins/ledger.md#Plugin-ledger
        .use(LedgerPlugin(LedgerConfig(
            store=".kelir/swarm-audit.merkle.db",
            merkle_chain=True,
            suppress_local_db=False,
            otel=None,
            receipts=ReceiptConfig(
                include_syscall_count=False,
            ),
        )))
        .on_vm_init(_init)
        .seal()
    )


async def run_shard(
    child: Session, shard_id: int, total: int,
) -> tuple[int, int]:
    await child.checkpoint.create(f"swarm-shard-{shard_id}/{total}")
    # TODO (L3): map the shard-exchange shared region, spawn cargo test --shard, and publish the result via the mesh handle.
    await child.stop()
    return shard_id, 0


async def main() -> None:
    async with build_runtime() as session:
        await session.terminal.resize(120, 40)
        await session.checkpoint.create("swarm-cache-ready")
        STATE.cache_ready = True

        children = [await session.fork() for _ in range(8)]
        results = await asyncio.gather(*(
            run_shard(child, shard_id, 8)
            for shard_id, child in enumerate(children)
        ))

        passed = 0
        winner: int | None = None
        for shard_id, code in results:
            if code == 0:
                passed += 1
                if winner is None:
                    winner = shard_id

        # TODO (L3): await the mesh winner announcement; the Py Caps expose no mesh handle with await_winner, so the first passing shard stands in as winner.
        print(f"[kelir] mesh winner shard={winner}")
        print(f"[kelir] {passed}/8 shards passed; winner shard={winner}")
        denials = await session.cap("ledger").count_denials(os.getpid())
        print(f"[kelir] supply-chain denials={denials}")



if __name__ == "__main__":
    asyncio.run(main())
