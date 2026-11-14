# see: cans/plugins/profile.md#Plugin-profile
from __future__ import annotations

from dataclasses import dataclass

from kelir.plugin_model import BasePlugin

__all__ = [
    "CdiConfig",
    "ProfileConfig",
    "ProfilePlugin",
]


@dataclass
class CdiConfig:
    enabled: bool = False
    spec_dir: str = ""


@dataclass
class ProfileConfig:
    rfc6902_dynamic_patching: bool = False
    enforce_monotonic_patches: bool = False
    cdi: CdiConfig | None = None


class ProfilePlugin(BasePlugin[None, None]):
    name = "profile"

    def __init__(self, config: ProfileConfig | None = None) -> None:
        self.config = config if config is not None else ProfileConfig()


# see: cans/02-plugin-model.md#Unified-builder 
from kelir import KelirBuilder


def profile_builder(config: ProfileConfig | None = None) -> KelirBuilder:
    cfg = config if config is not None else ProfileConfig()
    return KelirBuilder.plugin("profile").context(lambda: cfg)


# see: cans/02-plugin-model.md#Unified-builder
def profile(config: ProfileConfig | None = None) -> KelirBuilder:
    return profile_builder(config)
