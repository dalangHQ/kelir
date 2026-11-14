# see: cans/plugins/mesh.md#Plugin-mesh
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Literal

from kelir.plugin_model import BasePlugin

__all__ = [
    "MeshConfig",
    "MeshPlugin",
    "PeerIdentity",
    "PeerListener",
]


@dataclass
class PeerListener:
    port: int = 0
    service: str = ""


@dataclass
class PeerIdentity:
    trust_domain: str = ""
    workload_prefix: str = ""


@dataclass
class MeshConfig:
    peer_identity: PeerIdentity = field(default_factory=lambda: PeerIdentity())
    mtls: bool = False
    traceparent: Literal["inject-and-validate"] = "inject-and-validate"
    listeners: list[PeerListener] = field(default_factory=list)


class MeshPlugin(BasePlugin[None, None]):
    name = "mesh"

    def __init__(self, config: MeshConfig | None = None) -> None:
        self.config = config if config is not None else MeshConfig()


# see: cans/02-plugin-model.md#Unified-builder 
from kelir import KelirBuilder


def mesh_builder(config: MeshConfig | None = None) -> KelirBuilder:
    cfg = config if config is not None else MeshConfig()
    return KelirBuilder.plugin("mesh").context(lambda: cfg)


# see: cans/02-plugin-model.md#Unified-builder
def mesh(config: MeshConfig | None = None) -> KelirBuilder:
    return mesh_builder(config)
