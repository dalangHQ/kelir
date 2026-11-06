# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

from dataclasses import dataclass

from kelir.plugin import BasePlugin

__all__ = ["TransactionConfig", "TransactionPlugin"]


@dataclass
class TransactionConfig:
    timeout_ms: int = 0
    auto_rollback_on_taint: bool = False
    flush_fuse_before_commit: bool = False


class TransactionPlugin(BasePlugin):
    name = "transaction"

    def __init__(self, config: TransactionConfig | None = None) -> None:
        self.config = config if config is not None else TransactionConfig()
