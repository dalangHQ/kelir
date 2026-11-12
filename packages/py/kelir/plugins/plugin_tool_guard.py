# see: cans/plugins/tool-guard.md#Plugin-tool-guard
from __future__ import annotations

from dataclasses import dataclass

from kelir.model_plugin import BasePlugin

__all__ = [
    "ToolGuardConfig",
    "ToolGuardPlugin",
]


@dataclass
class ToolGuardConfig:
    max_invocations_per_trace: int = 50
    pin_schemas: bool = True


class ToolGuardPlugin(BasePlugin[None, None]):
    name = "tool-guard"

    def __init__(self, config: ToolGuardConfig | None = None) -> None:
        self.config = config if config is not None else ToolGuardConfig()


# see: cans/02-plugin-model.md#Unified-builder 
from kelir import KelirBuilder


def tool_guard_builder(config: ToolGuardConfig | None = None) -> KelirBuilder:
    cfg = config if config is not None else ToolGuardConfig()
    return KelirBuilder.plugin("tool-guard").context(lambda: cfg)
