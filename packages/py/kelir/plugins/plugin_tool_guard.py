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


class ToolGuardPlugin(BasePlugin):
    name = "tool-guard"

    def __init__(self, config: ToolGuardConfig | None = None) -> None:
        self.config = config if config is not None else ToolGuardConfig()


# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder 
from kelir.plugin_builder import PluginBuilder, _PluginWrapper


def tool_guard_builder(config: ToolGuardConfig | None = None) -> _PluginWrapper:
    return PluginBuilder("tool-guard").build()
