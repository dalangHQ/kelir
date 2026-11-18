# see: cans/plugins/taint.md#Plugin-taint
from __future__ import annotations

from dataclasses import dataclass

from kelir.plugin_model import BasePlugin

__all__ = [
    "TaintConfig",
    "TaintPlugin",
    "taint",
    "taint_builder",
]


@dataclass
class TaintConfig:
    pass


class TaintPlugin(BasePlugin[None, None]):
    name = "taint"

    def __init__(self, config: TaintConfig | None = None) -> None:
        self.config = config if config is not None else TaintConfig()


# see: cans/02-plugin-model.md#Unified-builder 
from kelir import KelirBuilder


def taint_builder(config: TaintConfig | None = None) -> KelirBuilder:
    cfg = config if config is not None else TaintConfig()
    return KelirBuilder.plugin("taint").context(lambda: cfg)


# see: cans/02-plugin-model.md#Unified-builder
def taint(config: TaintConfig | None = None) -> KelirBuilder:
    return taint_builder(config)
