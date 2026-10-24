# examples/db-backed-runtime.py
"""
Database-backed secure runtime. pgwire traffic is rewritten at the L7
boundary; credentials are injected per-connection host-side and never
enter guest address space. Cold start imports a portable .kelir-snap
from S3 via FastCDC streaming; OCI image layers are translated to EROFS
blocks in flight during the same boot.

Mounts:     db-proxy, cas-snapshot, oci-translator.
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
from kelir.plugins.cas_snapshot import (
    CasSnapshotConfig, CasSnapshotPlugin, FastCDCConfig, S3Tier,
)
from kelir.plugins.db_proxy import (
    DbProxyConfig, DbProxyPlugin, EphemeralCredentials,
    PgWireRewrite, SqliteOverVsock,
)
from kelir.plugins.oci_translator import (
    LayerPolicy, OciTranslatorConfig, OciTranslatorPlugin,
)


class MigrateParams(BaseModel, frozen=True):
    action: Literal["migrate"]
    schema_name: str
    target_version: int = Field(ge=1)
    dry_run: bool = False


def build_runtime(s3_bucket: str, pg_host: str, pg_secret: str) -> KelirBuilder:
    return (
        kelir.builder(
            vcpus=2, memory_mb=2048, driver="auto",
            kernel=f"./fixtures/vmlinux-6.6-{os.uname().machine}-kelir",
            rootfs_erofs="./fixtures/rootfs-migrate.erofs",
            tsc_locked=True, enable_watchdog=True,
        )

        # OCI layers → EROFS blocks in flight. The first workload exec
        # runs while the final layer flushes; chaos can fault-inject
        # into image pulls without a separate pull step.
        .plugin(OciTranslatorPlugin(OciTranslatorConfig(
            image="registry.internal/app-migrations:latest",
            layer_policy=LayerPolicy(
                flatten=True, apply_whiteouts=True,
                etc_overlay=True, output="erofs-block",
            ),
            in_flight=True,
        )))

        # FastCDC chunking + streaming S3 hydration. The <50 ms budget
        # is the invariant; if S3 is slower, cold start degrades to an
        # empty-RAM boot and re-uploads the snapshot.
        .plugin(CasSnapshotPlugin(CasSnapshotConfig(
            chunking=FastCDCConfig(
                min_bytes=16 * 1024,
                avg_bytes=64 * 1024,
                max_bytes=256 * 1024,
            ),
            tier=S3Tier(
                bucket=s3_bucket,
                prefix="snapshots/migrate/",
                hydration_budget_ms=50,
            ),
        )))

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

        .exec(_register_exec_graph)
    )


def _register_exec_graph(cmd: kelir.ExecChain) -> None:
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

    cmd.before("migrate *", pre_migrate)
    cmd.after("migrate *", post_migrate)


async def main() -> None:
    s3_bucket = os.environ["KELIR_S3_BUCKET"]
    pg_host = os.environ["KELIR_PG_HOST"]
    pg_secret = os.environ["KELIR_PG_SECRET"]

    async with build_runtime(s3_bucket, pg_host, pg_secret) as session:
        # Cold start: hydrate registers and dirty pages from the S3
        # snapshot in <50 ms. The first workload exec runs against warm
        # memory, not empty RAM.
        await session.vm.import_snapshot(
            session.cas.stream_from_s3("snapshots/migrate/latest.kelir-snap"),
        )

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

        # Export the post-migration state back to S3. The next cold
        # start hydrates it via the same cas-snapshot path.
        out = await session.vm.export_snapshot()
        await session.cas.upload_to_s3(
            out, "snapshots/migrate/latest.kelir-snap",
        )

        final = await session.ledger.finalize(
            include_merkle_root=True,
            include_delta=await session.get_causal_delta_from_parent(),
        )
        print(f"[kelir] merkle={final.merkle_root} jws={final.signed_jws[:64]}…")


if __name__ == "__main__":
    asyncio.run(main())
