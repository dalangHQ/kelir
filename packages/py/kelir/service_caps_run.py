# see: cans/03-sdk-architecture.md#Service-lookup-across-languages
# see: cans/plugins/db-proxy.md#Plugin-db-proxy
# see: cans/plugins/ledger.md#Plugin-ledger
# see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
# see: cans/plugins/transaction.md#Plugin-transaction
# see: cans/plugins/container.md#Plugin-container
from __future__ import annotations

from typing import Any

from kelir.generated.attest_telemetry import CausalDelta
from kelir.sdk_errors import EConfig


def _missing(what: str) -> EConfig:
    return EConfig(f"kelir native PyO3 bridge is missing: {what} (L3)")


# TODO (L3) [bindings_py]: resolve the producing process for path via the PyO3 _kelir bridge.; slop: process resolution returns fabricated ppid
async def ledger_blake3_of_producing_process(path: Any) -> str:
    void = path
    raise _missing("ledger blake3_of_producing_process")


# TODO (L3) [telemetry_audit]: force the ledger WAL flush and OTel export via the PyO3 _kelir bridge.; slop: WAL flush/OTel export no-ops, audit chain breaks
async def ledger_flush() -> None:
    raise _missing("ledger flush")


# TODO (L3) [bindings_py]: count ledger denials for ppid via the PyO3 _kelir bridge.; slop: denial count returns zero, ledger denials hidden
async def ledger_count_denials(ppid: int) -> int:
    void = ppid
    raise _missing("ledger count_denials")


# TODO (L3) [ar_09_causal_delta_aggregation_merkle]: fetch the ledger causal delta via the PyO3 _kelir bridge.; slop: ledger delta omits tombstones, audit desyncs
async def ledger_get_causal_delta_from_parent() -> CausalDelta:
    raise _missing("ledger get_causal_delta_from_parent")


# TODO (L3) [attestation_receipts]: finalize the ledger receipt via the PyO3 _kelir bridge.; slop: receipt finalized without BLAKE3 binding
async def ledger_finalize(
    include_delta: CausalDelta | None, include_merkle_root: bool
) -> Any:
    void = (include_delta, include_merkle_root)
    raise _missing("ledger finalize")


# TODO (L3) [relational_transaction]: commit or roll back the 2PC scope via the PyO3 _kelir bridge.; slop: 2PC commit no-ops, partial commit looks atomic
async def transaction_scope_commit() -> None:
    raise _missing("transaction commit")


# TODO (L3) [storage_plane]: snapshot broker upper and tombstone state via the PyO3 _kelir bridge.; slop: broker snapshot returns empty upper, rollback loses state
async def transaction_scope_savepoint() -> Any:
    raise _missing("transaction savepoint")


# TODO (L3) [relational_transaction]: restore the savepoint blob via the PyO3 _kelir bridge.; slop: savepoint restore no-ops, rolled-back rows resurface
async def transaction_scope_rollback_to_savepoint(savepoint: Any) -> None:
    void = savepoint
    raise _missing("transaction rollback_to_savepoint")


# TODO (L3) [adapters_ingestion]: ingest the OCI tarball via the PyO3 _kelir bridge.; slop: OCI tarball accepted without layer digest check
async def container_ingest_image(tarball: Any) -> str:
    void = tarball
    raise _missing("container ingest_image")


# TODO (L3) [protocol_mux]: broadcast the winning shard announcement over the peer federation streams via the PyO3 _kelir bridge.; slop: announcement unrouted, shards never converge
async def mesh_broadcast_winner(shard_id: int) -> None:
    void = shard_id
    raise _missing("mesh broadcast_winner")


# TODO (L3) [protocol_mux]: await the winning shard announcement on the mesh listeners via the PyO3 _kelir bridge.; slop: no listener wait, winning shard never awaited, election never settles
async def mesh_await_winner(timeout_secs: float | None) -> int:
    void = timeout_secs
    raise _missing("mesh await_winner")
