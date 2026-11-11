# see: cans/plugins/vault.md#Plugin-vault
from __future__ import annotations

from dataclasses import dataclass

from kelir.model_plugin import BasePlugin

__all__ = [
    "VaultConfig",
    "VaultPlugin",
]


@dataclass
class VaultConfig:
    metadata_vsock_port: int = 80
    token_ttl_seconds: int = 60


class VaultPlugin(BasePlugin):
    name = "vault"

    def __init__(self, config: VaultConfig | None = None) -> None:
        self.config = config if config is not None else VaultConfig()


# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder 
from kelir.plugin_builder import PluginBuilder, _PluginWrapper


def vault_builder(config: VaultConfig | None = None) -> _PluginWrapper:
    cfg = config if config is not None else VaultConfig()
    return PluginBuilder("vault").context(lambda: cfg).build()
