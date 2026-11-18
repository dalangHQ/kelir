# see: cans/plugins/ledger.md#Plugin-ledger
from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum

from kelir.plugin_model import BasePlugin

__all__ = [
    "HashingStandard",
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


# see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.hashing_standard
class HashingStandard(str, Enum):
    RFC_6962_DUAL_BLAKE3_SHA256 = "RFC_6962_DUAL_BLAKE3_SHA256"


@dataclass
class LedgerConfig:
    # see: cans/artifacts/examples/fixtures/kelir.yaml#plugins.ledger.merkle_wal.store_path
    store_path: str = ""
    merkle_chain: bool = False
    suppress_local_db: bool = False
    hashing_standard: HashingStandard = HashingStandard.RFC_6962_DUAL_BLAKE3_SHA256
    record_pre_redaction_hash: bool = False
    include_hook_spans_in_root: bool = False
    witness_cosigning_endpoint: str | None = None
    otel: OtelConfig | None = None
    receipts: ReceiptConfig = field(default_factory=lambda: ReceiptConfig())


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
