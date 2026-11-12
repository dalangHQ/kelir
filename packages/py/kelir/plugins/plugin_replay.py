# see: cans/plugins/replay.md#Plugin-replay
from __future__ import annotations

from dataclasses import dataclass, field

from kelir.model_plugin import BasePlugin

__all__ = [
    "ReplayConfig",
    "ReplayPlugin",
]


@dataclass
class ReplayConfig:
    step_window: int = 1_000_000
    kernel_params: list[str] = field(default_factory=lambda: ["clocksource=kvm-clock", "tsc=reliable", "nohpet"])
    deterministic_rng_seed: int = 0x5EED
    network_isolation: bool = True


class ReplayPlugin(BasePlugin[None, None]):
    name = "replay"

    def __init__(self, config: ReplayConfig | None = None) -> None:
        self.config = config if config is not None else ReplayConfig()


# see: cans/02-plugin-model.md#Unified-builder 
from kelir import KelirBuilder


def replay_builder(config: ReplayConfig | None = None) -> KelirBuilder:
    cfg = config if config is not None else ReplayConfig()
    return KelirBuilder.plugin("replay").context(lambda: cfg)
