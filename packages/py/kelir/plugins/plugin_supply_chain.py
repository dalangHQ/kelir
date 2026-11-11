# see: cans/plugins/supply-chain.md#Plugin-supply-chain
from __future__ import annotations

from dataclasses import dataclass, field

from kelir.model_plugin import BasePlugin

__all__ = [
    "AuthorizedJitEngine",
    "PackageGuardConfig",
    "SupplyChainConfig",
    "SupplyChainPlugin",
    "WxJitConfig",
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
    shannon_entropy_threshold_bits: float = 5.2
    bypass_runners: list[str] = field(default_factory=lambda: ["node", "python"])


@dataclass
class SupplyChainConfig:
    wx_jit: WxJitConfig = field(default_factory=lambda: WxJitConfig())
    package_guard: PackageGuardConfig = field(default_factory=lambda: PackageGuardConfig())


class SupplyChainPlugin(BasePlugin):
    name = "supply-chain"

    def __init__(self, config: SupplyChainConfig | None = None) -> None:
        self.config = config if config is not None else SupplyChainConfig()


# see: cans/02-plugin-model.md#Unified-builder 
from kelir import KelirBuilder


def supply_chain_builder(config: SupplyChainConfig | None = None) -> KelirBuilder:
    cfg = config if config is not None else SupplyChainConfig()
    return KelirBuilder.plugin("supply-chain").context(lambda: cfg)
