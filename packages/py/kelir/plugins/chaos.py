# see: cans/plugins/chaos.md#Plugin-chaos
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Literal

from kelir.plugin_model import BasePlugin

__all__ = [
    "ChaosConfig",
    "ChaosPlugin",
    "NetworkFaultRule",
    "StorageFaultRule",
    "chaos",
    "chaos_builder",
]


@dataclass
class NetworkFaultRule:
    target: str = ""
    direction: Literal["outbound", "inbound", "both"] = "outbound"
    latency_ms: int = 0
    jitter_ms: int = 0
    drop_rate: float = 0.0
    inject_rst: bool = False
    bandwidth_kbps: int | None = None


@dataclass
class StorageFaultRule:
    target_glob: str = ""
    simulate_errno: Literal["eperm", "eacces", "enoent", "einval", "enospc", "erofs", "econnrefused", "enxio", "ewouldblock", "efault", "ebusy", "exdev", "epipe", "enodata", "eproto", "epolicy", "etoolquota", "ewxviolation", "econfig", "esecurityviolation", "eunknownkind", "eclockregress", "path-traversal", "etimeout", "eafnosupport"] = "eperm"
    probability: float = 0.0


@dataclass
class ChaosConfig:
    network: list[NetworkFaultRule] = field(default_factory=list)
    storage: list[StorageFaultRule] = field(default_factory=list)


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
