# see: cans/plugins/l7-http.md#Plugin-l7-http
from __future__ import annotations

from dataclasses import dataclass, field

from kelir.plugin_model import BasePlugin

__all__ = [
    "L7HttpConfig",
    "L7HttpPlugin",
    "SinkholeConfig",
    "SinkholeRule",
    "l7_http",
    "l7_http_builder",
]


@dataclass
class SinkholeRule:
    host: str = ""
    methods: list[str] | None = None
    paths: list[str] | None = None
    allow: bool = False


@dataclass
class SinkholeConfig:
    rules: list[SinkholeRule] = field(default_factory=list)


@dataclass
class L7HttpConfig:
    sinkhole: SinkholeConfig = field(default_factory=lambda: SinkholeConfig())
    protocol_inspection: bool = False


class L7HttpPlugin(BasePlugin[None, None]):
    name = "l7-http"

    def __init__(self, config: L7HttpConfig | None = None) -> None:
        self.config = config if config is not None else L7HttpConfig()


# see: cans/02-plugin-model.md#Unified-builder 
from kelir import KelirBuilder


def l7_http_builder(config: L7HttpConfig | None = None) -> KelirBuilder:
    cfg = config if config is not None else L7HttpConfig()
    return KelirBuilder.plugin("l7-http").context(lambda: cfg)


# see: cans/02-plugin-model.md#Unified-builder
def l7_http(config: L7HttpConfig | None = None) -> KelirBuilder:
    return l7_http_builder(config)
