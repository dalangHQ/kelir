# see: cans/plugins/container.md#Plugin-container
from __future__ import annotations

from dataclasses import dataclass

from kelir.model_plugin import BasePlugin

__all__ = [
    "ContainerConfig",
    "ContainerPlugin",
]


@dataclass
class ContainerConfig:
    runtime: str = "crun"
    isolate_var_lib_containers: bool = True
    allow_unsandboxed: bool = False
    pin_subuid_ranges: bool = True
    scratch_device: str | None = None


class ContainerPlugin(BasePlugin):
    name = "container"

    def __init__(self, config: ContainerConfig | None = None) -> None:
        self.config = config if config is not None else ContainerConfig()


# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder 
from kelir.plugin_builder import PluginBuilder, _PluginWrapper


def container_builder(config: ContainerConfig | None = None) -> _PluginWrapper:
    cfg = config if config is not None else ContainerConfig()
    return PluginBuilder("container").context(lambda: cfg).build()
