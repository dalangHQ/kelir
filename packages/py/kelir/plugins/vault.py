# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

from dataclasses import dataclass

from kelir.plugin import BasePlugin

__all__ = ["VaultConfig", "VaultPlugin"]


@dataclass
class VaultConfig:
    metadata_vsock_port: int = 0
    token_ttl_seconds: int = 0


class VaultPlugin(BasePlugin):
    name = "vault"

    def __init__(self, config: VaultConfig | None = None) -> None:
        self.config = config if config is not None else VaultConfig()
