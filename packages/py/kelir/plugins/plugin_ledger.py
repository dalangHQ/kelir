# see: cans/plugins/ledger.md#Plugin-ledger
from __future__ import annotations

from dataclasses import dataclass, field

from kelir.model_plugin import BasePlugin

__all__ = [
    "LedgerConfig",
    "LedgerPlugin",
    "OtelConfig",
    "ReceiptConfig",
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
    store: str = ""
    merkle_chain: bool = False
    otel: OtelConfig | None = None
    receipts: ReceiptConfig = field(default_factory=lambda: ReceiptConfig())


class LedgerPlugin(BasePlugin):
    name = "ledger"

    def __init__(self, config: LedgerConfig | None = None) -> None:
        self.config = config if config is not None else LedgerConfig()


# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder 
from kelir.plugin_builder import PluginBuilder, _PluginWrapper


def ledger_builder(config: LedgerConfig | None = None) -> _PluginWrapper:
    return PluginBuilder("ledger").build()
