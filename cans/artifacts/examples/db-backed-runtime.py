# see: cans/plugins/db-proxy.md#Plugin-db-proxy
# see: cans/plugins/ledger.md#Plugin-ledger
# see: cans/plugins/transaction.md#Plugin-transaction
from __future__ import annotations

import asyncio
import os
from typing import Literal

from pydantic import BaseModel, Field

import kelir
from kelir import HandlerContext, KelirBuilder, PostExecAction, Session, TrapFilter, Verdict
from kelir.plugins.plugin_db_proxy import (
    DbProxyConfig, DbProxyPlugin, EphemeralCredentials,
    PgWireRewrite, SqliteOverVsock,
)
from kelir.plugins.plugin_ledger import (
    LedgerConfig, LedgerPlugin, ReceiptConfig,
)
from kelir.plugins.plugin_transaction import (
    TransactionConfig, TransactionPlugin,
)


class MigrateParams(BaseModel, frozen=True):
    action: Literal["migrate"]
    schema_name: str
    target_version: int = Field(ge=1)
    dry_run: bool = False


LOCAL_SNAPSHOT = "./fixtures/migrate-snapshot.erofs"


def build_runtime(pg_host: str, pg_secret: str) -> KelirBuilder:
    builder = (
        kelir.builder(
            vcpus=2, memory_mb=2048, driver="auto",
            kernel=f"./fixtures/vmlinux-6.6-{os.uname().machine}-kelir",
            rootfs_erofs="./fixtures/rootfs-migrate.erofs",
            tsc_locked=True, enable_watchdog=True,
        )

        # see: cans/plugins/db-proxy.md#L7-pgwire-rewrite-and-2PC-commit
        .plugin(DbProxyPlugin(DbProxyConfig(
            postgres=PgWireRewrite(
                listen_port=5432,
                upstream=f"{pg_host}:5432",
                rewrite_rules=[
                    ("public.", "migrate_schema."),
                ],
            ),
            # see: cans/plugins/db-proxy.md#Ephemeral-credentials-injected-at-host-connection-boundary
            credentials=EphemeralCredentials(
                placeholder="vault://pg/app_migrate",
                ttl_seconds=300,
            ),
            sqlite=SqliteOverVsock(
                guest_socket="/run/kelir/db.sock",
                host_path="/var/lib/app/migrations.sqlite",
            ),
        )))

        # see: cans/plugins/ledger.md#Merkle-WAL
        .plugin(LedgerPlugin(LedgerConfig(
            store="sqlite://.kelir/migrate-audit.merkle.db",
            merkle_chain=True,
            otel=None,
            receipts=ReceiptConfig(
                include_syscall_count=False,
            ),
        )))

        # see: cans/plugins/transaction.md#Two-phase-commit-demonstration
        .plugin(TransactionPlugin(TransactionConfig(
            timeout_ms=30000,
            auto_rollback_on_taint=True,
            flush_fuse_before_commit=True,
        )))
    )

    async def pre_migrate(c: HandlerContext) -> Verdict:
        if c.cap.db_proxy.rewrite_target != "migrate_schema.":
            return Verdict.drop(
                "EdbProxySchema",
                f"EdbProxySchema: got {c.cap.db_proxy.rewrite_target!r}",
            )
        return Verdict.pass_()

    async def post_migrate(c: HandlerContext) -> PostExecAction:
        receipt = await c.receipt()
        if not any(e.namespace == "net:l7:sql" for e in receipt.extensions):
            return PostExecAction.rollback_and_halt()
        return PostExecAction.proceed()

    # see: cans/plugins/db-proxy.md#Plugin-db-proxy
    builder.trap(TrapFilter.binary("migrate"), pre=pre_migrate, post=post_migrate)

    # see: cans/plugins/transaction.md#Plugin-transaction
    @builder.on_pre_commit
    async def _pre_commit(session: Session) -> None:
        await session.cap.ledger.flush()

    @builder.on_post_commit
    async def _post_commit(session: Session, committed: bool) -> None:
        if committed:
            print("[kelir] commit: per-entry leaf commit, staged snapshot "
                  "is next cold-start fixture")
        else:
            print("[kelir] commit aborted: staging unlinked, broker upper "
                  "and tombstones dropped, last good snapshot kept")

    return builder


async def main() -> None:
    pg_host = os.environ["KELIR_PG_HOST"]
    pg_secret = os.environ["KELIR_PG_SECRET"]

    async with build_runtime(pg_host, pg_secret) as session:
        # see: cans/09-snapshots.md#Snapshots
        with open(LOCAL_SNAPSHOT, "rb") as f:
            await session.vm.import_snapshot(f.read())

        # see: cans/plugins/transaction.md#Temporal-savepoints
        async with session.cap.transaction.scope(
            "migrate-42",
            paths=["/workspace/migrations"],
            schemas=["migrate_schema"],
        ) as tx:
            sp = await tx.savepoint()
            run = await session.spawn(
                "python", ["-m", "app.migrate", "--schema", "migrate"],
                cwd="/workspace/migrations",
                params=MigrateParams(
                    action="migrate", schema_name="migrate",
                    target_version=42, dry_run=False,
                ),
            )
            if run.exit_code != 0:
                await tx.rollback_to_savepoint(sp)
                raise SystemExit(f"migration failed: {run.exit_code}")

            out = await session.vm.export_snapshot()
            with open(LOCAL_SNAPSHOT, "wb") as f:
                f.write(out)

        ledger = session.cap.ledger
        # see: cans/10-attestation.md#Execution-receipts
        final = await ledger.finalize(
            include_merkle_root=True,
            include_delta=await ledger.get_causal_delta_from_parent(),
        )
        print(f"[kelir] merkle={final.merkle_root} jws={final.signed_jws[:64]}…")


if __name__ == "__main__":
    asyncio.run(main())
