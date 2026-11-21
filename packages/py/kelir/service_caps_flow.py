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
from kelir.sdk_errors_flow import EConfig
from kelir.service_caps_run import (
    container_ingest_image as _run_container_ingest_image,
    ledger_blake3_of_producing_process as _run_ledger_blake3,
    ledger_count_denials as _run_ledger_count_denials,
    ledger_finalize as _run_ledger_finalize,
    ledger_flush as _run_ledger_flush,
    ledger_get_causal_delta_from_parent as _run_ledger_causal_delta,
    mesh_await_winner as _run_mesh_await_winner,
    mesh_broadcast_winner as _run_mesh_broadcast_winner,
    transaction_scope_commit as _run_transaction_commit,
    transaction_scope_rollback_to_savepoint as _run_transaction_rollback_to,
    transaction_scope_savepoint as _run_transaction_savepoint,
)

__all__ = [
    "Caps",
    "ContainerCap",
    "ContainerHandle",
    "DbCap",
    "DbProxyHandle",
    "LedgerCap",
    "LedgerFinalizeResult",
    "LedgerHandle",
    "MeshHandle",
    "RepoPolicyCap",
    "Savepoint",
    "SqlProxyHandle",
    "TransactionCap",
    "TransactionHandle",
    "TransactionScope",
    "Tx",
    "materialize_cap",
]



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
        return await _run_ledger_blake3(path)

    async def flush(self) -> None:
        return await _run_ledger_flush()

    async def count_denials(self, ppid: int) -> int:
        return await _run_ledger_count_denials(ppid)

    async def get_causal_delta_from_parent(self) -> CausalDelta:
        return await _run_ledger_causal_delta()

    async def finalize(
        self,
        *,
        include_delta: CausalDelta | None = None,
        include_merkle_root: bool = False,
    ) -> LedgerFinalizeResult:
        return await _run_ledger_finalize(include_delta, include_merkle_root)


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
        return await _run_transaction_commit()

    async def savepoint(self) -> Savepoint:
        return await _run_transaction_savepoint()

    async def rollback_to_savepoint(self, savepoint: Savepoint) -> None:
        return await _run_transaction_rollback_to(savepoint)

    async def commit(self) -> None:
        return await _run_transaction_commit()


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


# see: cans/03-sdk-architecture.md#Service-lookup-across-languages
TransactionHandle = TransactionCap
Tx = TransactionScope


class ContainerCap:
    async def ingest_image(self, tarball: Path | str) -> str:
        return await _run_container_ingest_image(tarball)


class MeshHandle:
    async def broadcast_winner(self, shard_id: int) -> None:
        return await _run_mesh_broadcast_winner(shard_id)

    async def await_winner(
        self,
        timeout: timedelta | float | int | None = None,
        *,
        timeout_secs: float | None = None,
    ) -> int:
        timeout_secs = timeout_secs if timeout_secs is not None else (
            float(timeout) if isinstance(timeout, (int, float)) else None
        )
        return await _run_mesh_await_winner(timeout_secs)


# see: cans/03-sdk-architecture.md#Service-lookup-across-languages
ContainerHandle = ContainerCap
DbProxyHandle = DbCap
SqlProxyHandle = DbCap
LedgerHandle = LedgerCap


class Caps:
    def __init__(self) -> None:
        self.db_proxy = DbCap()
        self.ledger = LedgerCap()
        self.mesh = MeshHandle()
        self.repo_policy = RepoPolicyCap()
        self.transaction = TransactionCap()
        self.container = ContainerCap()
        self.tool_guard: Any = None
        self.vault: Any = None
        self.replay: Any = None

    # see: cans/03-sdk-architecture.md#Service-lookup-across-languages
    def __call__(self, name: str) -> Any | None:
        key = _CAP_KEY_ALIASES.get(name, name)
        return getattr(self, key, None)


# see: cans/03-sdk-architecture.md#Service-lookup-across-languages
_CAP_KEY_ALIASES: dict[str, str] = {
    "db-proxy": "db_proxy",
    "repo-policy": "repo_policy",
    "tool-guard": "tool_guard",
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
