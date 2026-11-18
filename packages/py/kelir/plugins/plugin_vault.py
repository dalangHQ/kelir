# see: cans/plugins/vault.md#Plugin-vault
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Literal

from kelir.plugin_model import BasePlugin

__all__ = [
    "CredentialDescriptor",
    "CredentialKind",
    "VaultConfig",
    "VaultPlugin",
    "vault",
    "vault_builder",
]


# see: cans/plugins/vault.md#Credential-types
CredentialKind = Literal["bearer_token", "sig_v4", "mtls_bundle"]


@dataclass
class CredentialDescriptor:
    name: str = ""
    kind: CredentialKind = "bearer_token"
    audience: str = ""
    placeholder: str = ""


@dataclass
class VaultConfig:
    metadata_vsock_port: int = 80
    token_ttl_seconds: int = 60
    credentials: list[CredentialDescriptor] = field(default_factory=list)


class VaultPlugin(BasePlugin[None, None]):
    name = "vault"

    def __init__(self, config: VaultConfig | None = None) -> None:
        self.config = config if config is not None else VaultConfig()


# see: cans/02-plugin-model.md#Unified-builder 
from kelir import KelirBuilder


def vault_builder(config: VaultConfig | None = None) -> KelirBuilder:
    cfg = config if config is not None else VaultConfig()
    return KelirBuilder.plugin("vault").context(lambda: cfg)


# see: cans/02-plugin-model.md#Unified-builder
def vault(config: VaultConfig | None = None) -> KelirBuilder:
    return vault_builder(config)
