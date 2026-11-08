# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

from dataclasses import dataclass, field

from kelir.plugin import BasePlugin

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
    max_buffered_bytes: int = 65536
    idle_timeout_ms: int = 0


@dataclass
class WakeOnPacketConfig:
    transport_channel: int = 0


@dataclass
class BacklogConfig:
    max_connections: int = 0
    overflow: str = ""


@dataclass
class WireIngressConfig:
    listen: ListenConfig | None = None
    parking: ParkingConfig | None = None
    wake_on_packet: WakeOnPacketConfig | None = None
    backlog: BacklogConfig | None = None


class WireIngressPlugin(BasePlugin):
    name = "wire-ingress"

    def __init__(self, config: WireIngressConfig | None = None) -> None:
        self.config = config if config is not None else WireIngressConfig()
