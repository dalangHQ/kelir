# see: cans/plugins/profile.md#Plugin-profile
from __future__ import annotations

from dataclasses import dataclass

from kelir.model_plugin import BasePlugin

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


class ProfilePlugin(BasePlugin):
    name = "profile"

    def __init__(self, config: ProfileConfig | None = None) -> None:
        self.config = config if config is not None else ProfileConfig()
