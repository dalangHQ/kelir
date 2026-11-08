# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

from dataclasses import dataclass, field

from kelir.plugin import BasePlugin

__all__ = [
    "PackageGuardConfig",
    "SupplyChainConfig",
    "SupplyChainPlugin",
    "WxJitConfig",
]


@dataclass
class WxJitConfig:
    trap_memfd: bool = False
    trap_mprotect_exec: bool = False
    allowed_jit_binaries: list[str] = field(default_factory=list)


@dataclass
class PackageGuardConfig:
    deny_lifecycle_scripts: bool = False
    managers: list[str] = field(default_factory=list)
    shannon_entropy_threshold_bits: float = 0.0
    bypass_runners: list[str] = field(default_factory=list)


@dataclass
class SupplyChainConfig:
    wx_jit: WxJitConfig | None = None
    package_guard: PackageGuardConfig | None = None


class SupplyChainPlugin(BasePlugin):
    name = "supply-chain"

    def __init__(self, config: SupplyChainConfig | None = None) -> None:
        self.config = config if config is not None else SupplyChainConfig()
