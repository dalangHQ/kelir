# examples/db-backed-runtime.py
"""
Database-backed secure runtime. pgwire traffic is rewritten at the L7
boundary; credentials are injected per-connection host-side and never
enter guest address space. The guest root filesystem and the migration
snapshot are local EROFS fixtures under ./fixtures/; no remote image
translation or snapshot streaming is involved.

Mounts:     db-proxy.
Primitives: export_snapshot, import_snapshot.

Requirements:
    kelir>=3.0
    pydantic>=2.6
"""

from __future__ import annotations

import asyncio
import os
from typing import Literal

from pydantic import BaseModel, Field

import kelir
from kelir import Decision, HandlerContext, KelirBuilder, PostExecAction
from kelir.plugins.db_proxy import (
    DbProxyConfig, DbProxyPlugin, EphemeralCredentials,
    PgWireRewrite, SqliteOverVsock,
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

        # Local EROFS fixtures: the root filesystem above
        # (./fixtures/rootfs-migrate.erofs) and LOCAL_SNAPSHOT are
        # mounted/imported directly; no image translation step runs.
        # pgwire L7 rewrite + ephemeral creds + SQLite over vsock.
        .plugin(DbProxyPlugin(DbProxyConfig(
            postgres=PgWireRewrite(
                listen_port=5432,
                upstream=f"{pg_host}:5432",
                # Proxy parses Q/P frames, rewrites the schema prefix,
                # and re-encodes. Rewrite applies at the L7 boundary,
                # not inside the guest kernel.
                rewrite_rules=[
                    ("public.", "migrate_schema."),
                ],
            ),
            # Guest sees a vault:// URN; the proxy swaps in the real
            # password after TLS termination. TTL bounds exposure.
            credentials=EphemeralCredentials(
                placeholder="vault://pg/app_migrate",
                resolve=lambda: pg_secret,
                ttl_seconds=300,
            ),
            # SQLite over vsock: forwarded to a host service that opens
            # the real .sqlite file.
            sqlite=SqliteOverVsock(
                guest_socket="/run/kelir/db.sock",
                host_path="/var/lib/app/migrations.sqlite",
            ),
        )))
    )

    def pre_migrate(c: HandlerContext) -> Decision:
        # Pin the pgwire rewrite to the expected schema prefix. A drift
        # means the proxy config diverged from the migration source;
        # the correct action is deny, not retry.
        if c.db.rewrite_target != "migrate_schema.":
            return Decision.deny(
                1, f"EdbProxySchema: got {c.db.rewrite_target!r}",
            )
        return Decision.resume()

    def post_migrate(c: HandlerContext) -> PostExecAction:
        # A migration that opened zero SQL statements is either a no-op
        # or a falsified report; the ledger cannot distinguish them.
        if c.receipt().statement_count == 0:
            return PostExecAction.rollback_and_halt()
        return PostExecAction.proceed()

    builder.trap("migrate *", pre=pre_migrate, post=post_migrate)
    return builder


async def main() -> None:
    pg_host = os.environ["KELIR_PG_HOST"]
    pg_secret = os.environ["KELIR_PG_SECRET"]

    async with build_runtime(pg_host, pg_secret) as session:
        # Cold start: hydrate registers and dirty pages from the local
        # EROFS snapshot fixture. The first workload exec runs against
        # warm memory, not empty RAM.
        with open(LOCAL_SNAPSHOT, "rb") as f:
            await session.vm.import_snapshot(f.read())

        run = await session.spawn(
            "python", ["-m", "app.migrate", "--schema", "migrate"],
            cwd="/workspace/migrations",
            params=MigrateParams(
                action="migrate", schema_name="migrate",
                target_version=42, dry_run=False,
            ),
        )
        if run.exit_code != 0:
            raise SystemExit(f"migration failed: {run.exit_code}")

        # Export the post-migration state to the local fixture. The
        # next cold start hydrates it from the same local file.
        out = await session.vm.export_snapshot()
        with open(LOCAL_SNAPSHOT, "wb") as f:
            f.write(out)

        final = await session.ledger.finalize(
            include_merkle_root=True,
            include_delta=await session.get_causal_delta_from_parent(),
        )
        print(f"[kelir] merkle={final.merkle_root} jws={final.signed_jws[:64]}…")


if __name__ == "__main__":
    asyncio.run(main())
