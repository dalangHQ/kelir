# see: cans/plugins/chaos.md#Plugin-chaos
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Literal

from kelir.model_plugin import BasePlugin

__all__ = [
    "ChaosConfig",
    "ChaosPlugin",
    "NetworkFaultRule",
    "StorageFaultRule",
]


@dataclass
class NetworkFaultRule:
    target: str = ""
    direction: Literal["outbound", "inbound", "both"] = "outbound"
    base_ms: int = 0
    jitter_ms: int = 0
    drop_rate: float = 0.0
    inject_rst: bool = False
    bandwidth_kbps: int | None = None


@dataclass
class StorageFaultRule:
    target_path: str = ""
    simulate_error: Literal["eperm", "eacces", "enoent", "einval", "enospc", "erofs", "econnrefused", "enxio", "ewouldblock", "efault", "ebusy", "exdev", "epipe", "enodata", "eproto", "epolicy", "etoolquota", "ewxviolation", "econfig", "esecurityviolation", "eunknownkind", "eclockregress"] = "eperm"
    error_probability: float = 0.0


@dataclass
class ChaosConfig:
    network: list[NetworkFaultRule] = field(default_factory=list)
    fs: list[StorageFaultRule] = field(default_factory=list)


class ChaosPlugin(BasePlugin[None, None]):
    name = "chaos"

    def __init__(self, config: ChaosConfig | None = None) -> None:
        self.config = config if config is not None else ChaosConfig()


# see: cans/02-plugin-model.md#Unified-builder 
from kelir import KelirBuilder


def chaos_builder(config: ChaosConfig | None = None) -> KelirBuilder:
    cfg = config if config is not None else ChaosConfig()
    return KelirBuilder.plugin("chaos").context(lambda: cfg)


# see: cans/02-plugin-model.md#Unified-builder
def chaos(config: ChaosConfig | None = None) -> KelirBuilder:
    return chaos_builder(config)
