# see: cans/03-sdk-architecture.md#Python-SDK
"""Container plugin config + plugin class (stub)."""
from __future__ import annotations

from dataclasses import dataclass

from kelir.plugin import BasePlugin

__all__ = ["ContainerConfig", "ContainerPlugin"]


@dataclass
class ContainerConfig:
    runtime: str = ""
    isolate_var_lib_containers: bool = False
    pin_subuid_ranges: bool = False


class ContainerPlugin(BasePlugin):
    name = "container"

    def __init__(self, config: ContainerConfig | None = None) -> None:
        self.config = config if config is not None else ContainerConfig()
