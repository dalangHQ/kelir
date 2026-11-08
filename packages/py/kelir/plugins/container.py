# see: cans/plugins/container.md#Plugin-container
from __future__ import annotations

from dataclasses import dataclass

from kelir.plugin import BasePlugin

__all__ = [
    "ContainerConfig",
    "ContainerPlugin",
]


@dataclass
class ContainerConfig:
    runtime: str = "crun"
    isolate_var_lib_containers: bool = True
    pin_subuid_ranges: bool = True
    scratch_device: str | None = None


class ContainerPlugin(BasePlugin):
    name = "container"

    def __init__(self, config: ContainerConfig | None = None) -> None:
        self.config = config if config is not None else ContainerConfig()
