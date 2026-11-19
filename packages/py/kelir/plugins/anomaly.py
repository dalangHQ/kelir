# see: cans/plugins/anomaly.md#Plugin-anomaly
from __future__ import annotations

from dataclasses import dataclass

from kelir.plugin_model import BasePlugin

__all__ = [
    "AnomalyConfig",
    "AnomalyPlugin",
    "anomaly",
    "anomaly_builder",
]


@dataclass
class AnomalyConfig:
    sliding_window_depth: int = 300
    export_feature_vector: bool = True
    feature_extraction: bool = True
    surprisal_threshold: float = 1.0


class AnomalyPlugin(BasePlugin[None, None]):
    name = "anomaly"

    def __init__(self, config: AnomalyConfig | None = None) -> None:
        self.config = config if config is not None else AnomalyConfig()


# see: cans/02-plugin-model.md#Unified-builder 
from kelir import KelirBuilder


def anomaly_builder(config: AnomalyConfig | None = None) -> KelirBuilder:
    cfg = config if config is not None else AnomalyConfig()
    return KelirBuilder.plugin("anomaly").context(lambda: cfg)


# see: cans/02-plugin-model.md#Unified-builder
def anomaly(config: AnomalyConfig | None = None) -> KelirBuilder:
    return anomaly_builder(config)
