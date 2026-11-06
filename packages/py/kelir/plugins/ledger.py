# see: cans/03-sdk-architecture.md#Python-SDK
"""Ledger plugin config + plugin class (stub)."""
from __future__ import annotations

from dataclasses import dataclass

from kelir.plugin import BasePlugin

__all__ = ["LedgerConfig", "LedgerPlugin", "OtelConfig", "ReceiptConfig"]


@dataclass
class OtelConfig:
    endpoint: str = ""
    service_name: str = ""
    propagate_traceparent: bool = False


@dataclass
class ReceiptConfig:
    include_syscall_count: bool = False


@dataclass
class LedgerConfig:
    store: str = ""
    merkle_chain: bool = False
    otel: OtelConfig | None = None
    receipts: ReceiptConfig | None = None


class LedgerPlugin(BasePlugin):
    name = "ledger"

    def __init__(self, config: LedgerConfig | None = None) -> None:
        self.config = config if config is not None else LedgerConfig()
