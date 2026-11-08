# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

from dataclasses import dataclass, field

from kelir.plugin import BasePlugin

__all__ = ["ReplayConfig", "ReplayPlugin"]


@dataclass
class ReplayConfig:
    step_window: int = 0
    kernel_params: list[str] = field(default_factory=list)
    deterministic_rng_seed: int = 0
    network_isolation: bool = False


class ReplayPlugin(BasePlugin):
    name = "replay"

    def __init__(self, config: ReplayConfig | None = None) -> None:
        self.config = config if config is not None else ReplayConfig()
