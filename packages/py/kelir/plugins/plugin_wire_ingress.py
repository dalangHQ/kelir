# see: cans/plugins/wire-ingress.md#Plugin-wire-ingress
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Literal

from kelir.model_plugin import BasePlugin

__all__ = [
    "BacklogConfig",
    "ListenConfig",
    "ParkingConfig",
    "WakeOnPacketConfig",
    "WireIngressConfig",
    "WireIngressPlugin",
]


@dataclass
class ListenConfig:
    port: int = 0
    protocol: str = ""


@dataclass
class ParkingConfig:
    max_buffered_bytes: int = 0
    idle_timeout_ms: int = 0


@dataclass
class WakeOnPacketConfig:
    transport_channel: int = 0


@dataclass
class BacklogConfig:
    max_connections: int = 0
    overflow: Literal["reject-503"] = "reject-503"


@dataclass
class WireIngressConfig:
    listen: ListenConfig = field(default_factory=lambda: ListenConfig(port=8443, protocol="https"))
    parking: ParkingConfig = field(default_factory=lambda: ParkingConfig(max_buffered_bytes=64 * 1024, idle_timeout_ms=30_000))
    wake_on_packet: WakeOnPacketConfig = field(default_factory=lambda: WakeOnPacketConfig(transport_channel=0x0001))
    backlog: BacklogConfig = field(default_factory=lambda: BacklogConfig(max_connections=256, overflow="reject-503"))


class WireIngressPlugin(BasePlugin):
    name = "wire-ingress"

    def __init__(self, config: WireIngressConfig | None = None) -> None:
        self.config = config if config is not None else WireIngressConfig()


# see: cans/02-plugin-model.md#Unified-builder 
from kelir import KelirBuilder


def wire_ingress_builder(config: WireIngressConfig | None = None) -> KelirBuilder:
    cfg = config if config is not None else WireIngressConfig()
    return KelirBuilder.plugin("wire-ingress").context(lambda: cfg)
