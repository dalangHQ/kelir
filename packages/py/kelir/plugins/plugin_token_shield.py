# see: cans/plugins/token-shield.md#Plugin-token-shield
from __future__ import annotations

from dataclasses import dataclass
from typing import Literal

from kelir.plugin_model import BasePlugin

__all__ = [
    "TokenShieldConfig",
    "TokenShieldPlugin",
    "token_shield",
    "token_shield_builder",
]


@dataclass
class TokenShieldConfig:
    max_bytes_total: int = 131072
    max_lines_total: int = 500
    clamp_strategy: Literal["head-and-tail", "head-only", "tail-only"] = "head-and-tail"
    notice: str = "\\n\\u{26a0}\\u{fe0f} [SHIELD]: Output clamped to protect model context window.\\n"


class TokenShieldPlugin(BasePlugin[None, None]):
    name = "token-shield"

    def __init__(self, config: TokenShieldConfig | None = None) -> None:
        self.config = config if config is not None else TokenShieldConfig()


# see: cans/02-plugin-model.md#Unified-builder 
from kelir import KelirBuilder


def token_shield_builder(config: TokenShieldConfig | None = None) -> KelirBuilder:
    cfg = config if config is not None else TokenShieldConfig()
    return KelirBuilder.plugin("token-shield").context(lambda: cfg)


# see: cans/02-plugin-model.md#Unified-builder
def token_shield(config: TokenShieldConfig | None = None) -> KelirBuilder:
    return token_shield_builder(config)
