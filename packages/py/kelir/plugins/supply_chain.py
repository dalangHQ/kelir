# see: cans/plugins/supply-chain.md#Plugin-supply-chain
from __future__ import annotations

from dataclasses import dataclass, field

from kelir.plugin_model_flow import BasePlugin

__all__ = [
    "AuthorizedJitEngine",
    "PackageGuardConfig",
    "SupplyChainConfig",
    "SupplyChainPlugin",
    "WxJitConfig",
    "supply_chain",
    "supply_chain_builder",
]


@dataclass
class AuthorizedJitEngine:
    binary_path: str = ""
    binary_blake3: bytes = bytes(32)


@dataclass
class WxJitConfig:
    trap_memfd: bool = False
    trap_mprotect_exec: bool = False
    allowed_jit_binaries: list[AuthorizedJitEngine] = field(default_factory=list)


@dataclass
class PackageGuardConfig:
    deny_lifecycle_scripts: bool = True
    managers: list[str] = field(default_factory=lambda: ["npm", "pip", "cargo"])
    bypass_runners: list[str] = field(default_factory=lambda: ["node", "python"])
    resolve_lockfiles: bool = True
    advisories: list[str] = field(default_factory=lambda: ["osv", "snyk", "socket", "deps_dev"])
    strip_private_scopes: bool = True
    vet_skills: bool = True


@dataclass
class SupplyChainConfig:
    wx_jit: WxJitConfig = field(default_factory=lambda: WxJitConfig())
    package_guard: PackageGuardConfig = field(default_factory=lambda: PackageGuardConfig())


class SupplyChainPlugin(BasePlugin[None, None]):
    name = "supply-chain"

    def __init__(self, config: SupplyChainConfig | None = None) -> None:
        self.config = config if config is not None else SupplyChainConfig()


# see: cans/02-plugin-model.md#Unified-builder 
from kelir import KelirBuilder


def supply_chain_builder(config: SupplyChainConfig | None = None) -> KelirBuilder:
    cfg = config if config is not None else SupplyChainConfig()
    return KelirBuilder.plugin("supply-chain").context(lambda: cfg)


# see: cans/02-plugin-model.md#Unified-builder
def supply_chain(config: SupplyChainConfig | None = None) -> KelirBuilder:
    return supply_chain_builder(config)
