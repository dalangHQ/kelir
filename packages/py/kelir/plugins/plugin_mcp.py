# see: cans/plugins/mcp.md#Plugin-mcp
from __future__ import annotations

from dataclasses import dataclass

from kelir.model_plugin import BasePlugin

__all__ = [
    "McpConfig",
    "McpPlugin",
]


@dataclass
class McpConfig:
    pass


class McpPlugin(BasePlugin[None, None]):
    name = "mcp"

    def __init__(self, config: McpConfig | None = None) -> None:
        self.config = config if config is not None else McpConfig()


# see: cans/02-plugin-model.md#Unified-builder 
from kelir import KelirBuilder


def mcp_builder(config: McpConfig | None = None) -> KelirBuilder:
    cfg = config if config is not None else McpConfig()
    return KelirBuilder.plugin("mcp").context(lambda: cfg)
