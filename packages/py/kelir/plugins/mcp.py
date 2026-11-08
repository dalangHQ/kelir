# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

from dataclasses import dataclass, field

from kelir.plugin import BasePlugin

__all__ = ["McpConfig", "McpPlugin"]


@dataclass
class McpConfig:
    transport_stream_kind: str = ""
    framing: str = ""
    advertise_tools: list[str] = field(default_factory=list)


class McpPlugin(BasePlugin):
    name = "mcp"

    def __init__(self, config: McpConfig | None = None) -> None:
        self.config = config if config is not None else McpConfig()
