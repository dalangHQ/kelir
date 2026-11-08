# see: cans/plugins/mcp.md#Plugin-mcp
from __future__ import annotations

from dataclasses import dataclass

from kelir.plugin import BasePlugin

__all__ = [
    "McpConfig",
    "McpPlugin",
]


@dataclass
class McpConfig:
    pass


class McpPlugin(BasePlugin):
    name = "mcp"

    def __init__(self, config: McpConfig | None = None) -> None:
        self.config = config if config is not None else McpConfig()
