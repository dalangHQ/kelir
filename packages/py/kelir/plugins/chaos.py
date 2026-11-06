# see: cans/03-sdk-architecture.md#Python-SDK
"""Chaos plugin config + plugin class (stub)."""
from __future__ import annotations

from dataclasses import dataclass, field

from kelir.plugin import BasePlugin

__all__ = ["ChaosConfig", "ChaosPlugin", "FsChaos", "NetworkChaos"]


@dataclass
class NetworkChaos:
    target: str = ""
    base_ms: int = 0
    jitter_ms: int = 0
    drop_rate: float = 0.0
    direction: str = "outbound"


@dataclass
class FsChaos:
    target_path: str = ""
    simulate_error: str = ""
    error_probability: float = 0.0


@dataclass
class ChaosConfig:
    network: list[NetworkChaos] = field(default_factory=list)
    fs: list[FsChaos] = field(default_factory=list)


class ChaosPlugin(BasePlugin):
    name = "chaos"

    def __init__(self, config: ChaosConfig | None = None) -> None:
        self.config = config if config is not None else ChaosConfig()
