# see: cans/plugins/transaction.md#Plugin-transaction
from __future__ import annotations

from dataclasses import dataclass

from kelir.plugin import BasePlugin

__all__ = [
    "TransactionConfig",
    "TransactionPlugin",
]


@dataclass
class TransactionConfig:
    timeout_ms: int = 30_000
    auto_rollback_on_taint: bool = True
    flush_fuse_before_commit: bool = True


class TransactionPlugin(BasePlugin):
    name = "transaction"

    def __init__(self, config: TransactionConfig | None = None) -> None:
        self.config = config if config is not None else TransactionConfig()
