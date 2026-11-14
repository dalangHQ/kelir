# see: cans/03-sdk-architecture.md#Service-lookup-across-languages
# see: cans/plugins/db-proxy.md#Plugin-db-proxy
# see: cans/plugins/ledger.md#Plugin-ledger
# see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
# see: cans/plugins/transaction.md#Plugin-transaction
# see: cans/plugins/container.md#Plugin-container
from __future__ import annotations

from dataclasses import dataclass
from datetime import timedelta
from pathlib import Path
from types import SimpleNamespace
from typing import Any

from kelir.generated.attest_telemetry import CausalDelta
from kelir.sdk_errors import EConfig

__all__ = [
    "Caps",
    "ContainerCap",
    "DbCap",
    "LedgerCap",
    "LedgerFinalizeResult",
    "MeshHandle",
    "RepoPolicyCap",
    "Savepoint",
    "TransactionCap",
    "TransactionScope",
    "materialize_cap",
]


def _missing(what: str) -> EConfig:
    return EConfig(f"kelir native PyO3 bridge (_kelir) is missing: {what} (L3)")


@dataclass
class LedgerFinalizeResult:
    merkle_root: str = ""
    signed_jws: str = ""


@dataclass(frozen=True)
class Savepoint:
    token: str = ""


class DbCap:
    def __init__(self, rewrite_target: str = "") -> None:
        self.rewrite_target = rewrite_target


class LedgerCap:
    async def blake3_of_producing_process(self, path: Path | str) -> str:
        # TODO (L3) [bindings_py]: resolve the producing process for path via the PyO3 _kelir bridge.; slop: process resolution returns fabricated ppid
        raise _missing("ledger blake3_of_producing_process")

    async def flush(self) -> None:
        # TODO (L3) [telemetry_audit]: force the ledger WAL flush and OTel export via the PyO3 _kelir bridge.; slop: WAL flush/OTel export no-ops, audit chain breaks
        raise _missing("ledger flush")

    async def count_denials(self, ppid: int) -> int:
        # TODO (L3) [bindings_py]: count ledger denials for ppid via the PyO3 _kelir bridge.; slop: denial count returns zero, ledger denials hidden
        raise _missing("ledger count_denials")

    async def get_causal_delta_from_parent(self) -> CausalDelta:
        # TODO (L3) [ar_09_causal_delta_aggregation_merkle]: fetch the ledger causal delta via the PyO3 _kelir bridge.; slop: ledger delta omits tombstones, audit desyncs
        raise _missing("ledger get_causal_delta_from_parent")

    async def finalize(
        self,
        *,
        include_delta: CausalDelta | None = None,
        include_merkle_root: bool = False,
    ) -> LedgerFinalizeResult:
        # TODO (L3) [attestation_receipts]: finalize the ledger receipt via the PyO3 _kelir bridge.; slop: receipt finalized without BLAKE3 binding
        raise _missing("ledger finalize")


class RepoPolicyCap:
    def __init__(self, violations: int = 0) -> None:
        self.violations = violations


class TransactionScope:
    def __init__(
        self,
        name: str,
        paths: tuple[str, ...] = (),
        schemas: tuple[str, ...] = (),
    ) -> None:
        self.name = name
        self.paths = paths
        self.schemas = schemas

    async def __aenter__(self) -> TransactionScope:
        return self

    async def __aexit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: Any,
    ) -> None:
        # TODO (L3) [relational_transaction]: commit or roll back the 2PC scope via the PyO3 _kelir bridge.; slop: 2PC commit no-ops, partial commit looks atomic
        return None

    async def savepoint(self) -> Savepoint:
        # TODO (L3) [storage_plane]: snapshot broker upper and tombstone state via the PyO3 _kelir bridge.; slop: broker snapshot returns empty upper, rollback loses state
        raise _missing("transaction savepoint")

    async def rollback_to_savepoint(self, savepoint: Savepoint) -> None:
        # TODO (L3) [relational_transaction]: restore the savepoint blob via the PyO3 _kelir bridge.; slop: savepoint restore no-ops, rolled-back rows resurface
        raise _missing("transaction rollback_to_savepoint")


class TransactionCap:
    def scope(
        self,
        name: str,
        *,
        paths: list[str] | None = None,
        schemas: list[str] | None = None,
    ) -> TransactionScope:
        return TransactionScope(
            name, tuple(paths or ()), tuple(schemas or ())
        )


class ContainerCap:
    async def ingest_image(self, tarball: Path | str) -> str:
        # TODO (L3) [adapters_ingestion]: ingest the OCI tarball via the PyO3 _kelir bridge.; slop: OCI tarball accepted without layer digest check
        raise _missing("container ingest_image")


class MeshHandle:
    async def await_winner(self, timeout: timedelta) -> int:
        # TODO (L3) [protocol_mux]: await the winning shard announcement on the mesh listeners via the PyO3 _kelir bridge.; slop: no listener wait, winning shard never awaited, election never settles
        raise _missing("mesh await_winner")


class Caps:
    def __init__(self) -> None:
        self.db_proxy = DbCap()
        self.ledger = LedgerCap()
        self.mesh = MeshHandle()
        self.repo_policy = RepoPolicyCap()
        self.transaction = TransactionCap()
        self.container = ContainerCap()


# see: cans/03-sdk-architecture.md#Service-lookup-across-languages
_CAP_KEY_ALIASES: dict[str, str] = {
    "db-proxy": "db_proxy",
    "repo-policy": "repo_policy",
}


def materialize_cap(cap: Caps, plugin_name: str, record: dict[str, Any]) -> None:
    key = _CAP_KEY_ALIASES.get(plugin_name, plugin_name)
    target: Any = getattr(cap, key, None)
    if target is None:
        setattr(cap, key, SimpleNamespace(**record))
        return
    for field_name, value in record.items():
        if callable(getattr(target, field_name, None)):
            continue
        setattr(target, field_name, value)
