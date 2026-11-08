# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

from dataclasses import dataclass, field

from kelir.plugin import BasePlugin

__all__ = ["MeshConfig", "MeshPlugin", "PeerListener"]


@dataclass
class PeerListener:
    port: int = 0
    service: str = ""


@dataclass
class MeshConfig:
    trust_domain: str = ""
    workload_prefix: str = ""
    mtls: bool = False
    propagate_traceparent: bool = False
    listeners: list[PeerListener] = field(default_factory=list)


class MeshPlugin(BasePlugin):
    name = "mesh"

    def __init__(self, config: MeshConfig | None = None) -> None:
        self.config = config if config is not None else MeshConfig()
