# see: cans/plugins/redaction.md#Plugin-redaction
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Literal

from kelir.plugin_model import BasePlugin

__all__ = [
    "MaskReadConfig",
    "MaskReadRule",
    "RedactionConfig",
    "RedactionPlugin",
    "redaction",
    "redaction_builder",
]


@dataclass
class MaskReadRule:
    glob: str = ""
    patterns: list[tuple[str, str]] = field(default_factory=list)


@dataclass
class MaskReadConfig:
    engine: Literal["fast-regex", "streaming-automata"] = "fast-regex"
    carryover_bytes: int = 0
    rules: list[MaskReadRule] = field(default_factory=list)


@dataclass
class RedactionConfig:
    mask_read: MaskReadConfig = field(default_factory=lambda: MaskReadConfig())


class RedactionPlugin(BasePlugin[None, None]):
    name = "redaction"

    def __init__(self, config: RedactionConfig | None = None) -> None:
        self.config = config if config is not None else RedactionConfig()


# see: cans/02-plugin-model.md#Unified-builder 
from kelir import KelirBuilder


def redaction_builder(config: RedactionConfig | None = None) -> KelirBuilder:
    cfg = config if config is not None else RedactionConfig()
    return KelirBuilder.plugin("redaction").context(lambda: cfg)


# see: cans/02-plugin-model.md#Unified-builder
def redaction(config: RedactionConfig | None = None) -> KelirBuilder:
    return redaction_builder(config)
