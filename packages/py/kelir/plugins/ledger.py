# see: cans/plugins/ledger.md#Plugin-ledger
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Literal

from kelir.plugin_model_flow import BasePlugin

__all__ = [
    "LedgerConfig",
    "LedgerPlugin",
    "OtelConfig",
    "ReceiptConfig",
    "ledger",
    "ledger_builder",
]


@dataclass
class OtelConfig:
    endpoint: str = ""
    service_name: str = ""
    propagate_w3c_traceparent: bool = False
    export_diagnostic_pmu_gauges: bool = False


@dataclass
class ReceiptConfig:
    include_dirty_bytes: bool = False
    include_binary_blake3: bool = False
    include_syscall_count: bool = False
    include_wall_clock_ns: bool = False
    include_phase_timings: bool = False


@dataclass
class LedgerConfig:
    store_path: str = ""
    merkle_chain: bool = False
    suppress_local_db: bool = False
    hashing_standard: Literal["rfc-6962-dual-blake-3-sha-256"] = "rfc-6962-dual-blake-3-sha-256"
    record_pre_redaction_hash: bool = False
    include_hook_spans_in_root: bool = False
    witness_cosigning_endpoint: str | None = None
    otel: OtelConfig | None = None
    receipts: ReceiptConfig = field(default_factory=lambda: ReceiptConfig())
    strict: bool = False
    ocsf_mapping: bool = False


class LedgerPlugin(BasePlugin[None, None]):
    name = "ledger"

    def __init__(self, config: LedgerConfig | None = None) -> None:
        self.config = config if config is not None else LedgerConfig()


# see: cans/02-plugin-model.md#Unified-builder 
from kelir import KelirBuilder


def ledger_builder(config: LedgerConfig | None = None) -> KelirBuilder:
    cfg = config if config is not None else LedgerConfig()
    return KelirBuilder.plugin("ledger").context(lambda: cfg)


# see: cans/02-plugin-model.md#Unified-builder
def ledger(config: LedgerConfig | None = None) -> KelirBuilder:
    return ledger_builder(config)
