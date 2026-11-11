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


class ReplayPlugin(BasePlugin):
    name = "replay"

    def __init__(self, config: ReplayConfig | None = None) -> None:
        self.config = config if config is not None else ReplayConfig()


# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder 
from kelir.plugin_builder import PluginBuilder, _PluginWrapper


def replay_builder(config: ReplayConfig | None = None) -> _PluginWrapper:
    cfg = config if config is not None else ReplayConfig()
    return PluginBuilder("replay").context(lambda: cfg).build()
