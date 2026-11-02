# examples/db-backed-runtime.py
"""
Database-backed secure runtime. pgwire traffic is rewritten at the L7
boundary; credentials are scoped short-lived tokens issued by
plugin-vault and presented by the workload itself. No host TLS
termination by default, no CA injection. The guest root filesystem
and the migration snapshot are local EROFS fixtures under ./fixtures/;
no remote image translation or snapshot streaming is involved.

Mounts:     db-proxy, ledger.
Primitives: export_snapshot, import_snapshot.
Commit:     two-phase commit around the snapshot export — on_pre_commit
            pauses vCPUs and drains FUSE in-flight ops (the handler
            flushes the host ledger before staging); on_post_commit(True)
            is a per-entry leaf commit, on_post_commit(False) unlinks
            staging and drops the broker upper file and tombstones.

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
from kelir import HandlerContext, KelirBuilder, PostExecAction, Session, TrapFilter, Verdict
from kelir.plugins.db_proxy import (
    DbProxyConfig, DbProxyPlugin, EphemeralCredentials,
    PgWireRewrite, SqliteOverVsock,
)
from kelir.plugins.ledger import (
    LedgerConfig, LedgerPlugin, ReceiptConfig,
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
            # Guest sees a vault:// URN; the workload presents a scoped
            # short-lived token issued by plugin-vault itself. No host
            # TLS termination by default, no CA injection. TTL bounds
            # exposure.
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

        # Ledger: mounted so session.cap.ledger in _pre_commit and main()
        # resolves; without it the capability access fails at boot.
        .plugin(LedgerPlugin(LedgerConfig(
            store="sqlite://.kelir/migrate-audit.merkle.db",
            merkle_chain=True,
            otel=None,
            receipts=ReceiptConfig(
                include_syscall_count=False,
            ),
        )))
    )

    def pre_migrate(c: HandlerContext) -> Verdict:
        # Pin the pgwire rewrite to the expected schema prefix. A drift
        # means the proxy config diverged from the migration source;
        # the correct action is drop, not retry.
        if c.cap.db.rewrite_target != "migrate_schema.":
            return Verdict.drop(
                "EdbProxySchema",
                f"EdbProxySchema: got {c.cap.db.rewrite_target!r}",
            )
        return Verdict.pass_()

    def post_migrate(c: HandlerContext) -> PostExecAction:
        # A migration with no db-proxy telemetry is either a no-op
        # or a falsified report; the ledger cannot distinguish them.
        # Payload format is plugin-defined, so presence (not a decoded
        # count) is the gate: no "net:l7:sql" extension, no proceed.
        if not any(e.namespace == "net:l7:sql" for e in c.receipt().extensions):
            return PostExecAction.rollback_and_halt()
        return PostExecAction.proceed()

    builder.trap(TrapFilter.binary("migrate"), pre=pre_migrate, post=post_migrate)

    # ── TWO-PHASE COMMIT (plugins/transaction.md) ──────────
    # LifecycleParticipant commit hooks, registered the same decorator
    # way as on_vm_ready / on_vm_shutdown in autonomous-swe-agent.py.
    # They bracket the snapshot export in main(): the runtime pauses
    # vCPUs and drains FUSE in-flight ops, then these handlers run.
    @builder.on_pre_commit
    async def _pre_commit(session: Session) -> None:
        # Prepare phase: make the host side durable BEFORE the guest
        # snapshot is staged. A crash between export and ledger finalize
        # must not leave host DB state ahead of guest state — the exact
        # desynchronization the commit protocol exists to prevent.
        await session.cap.ledger.flush()

    @builder.on_post_commit
    async def _post_commit(session: Session, committed: bool) -> None:
        # Commit phase: committed=True is a per-entry leaf commit —
        # the broker replays the upper file + tombstones against the
        # base workspace, so the staged snapshot becomes the next
        # cold-start fixture; False unlinks staging and drops the
        # broker upper file and tombstones, so a failed export never
        # overwrites the last good snapshot.
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

        # Commit point (plugins/transaction.md): exporting the
        # post-migration state stages the snapshot; on_pre_commit already
        # paused vCPUs, drained FUSE in-flight ops, and flushed the host
        # ledger, and on_post_commit(True) per-entry leaf-commits the
        # staged snapshot into LOCAL_SNAPSHOT while on_post_commit(False)
        # unlinks staging and drops the broker upper file and tombstones,
        # leaving the last good fixture intact. The next cold start
        # hydrates from the same local file.
        out = await session.vm.export_snapshot()
        with open(LOCAL_SNAPSHOT, "wb") as f:
            f.write(out)

        ledger = session.cap.ledger
        final = await ledger.finalize(
            include_merkle_root=True,
            include_delta=await ledger.get_causal_delta_from_parent(),
        )
        print(f"[kelir] merkle={final.merkle_root} jws={final.signed_jws[:64]}…")


if __name__ == "__main__":
    asyncio.run(main())
