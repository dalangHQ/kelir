# see: cans/plugins/l7-http.md#Plugin-l7-http
from __future__ import annotations

from dataclasses import dataclass, field

from kelir.plugin_model import BasePlugin

__all__ = [
    "DeclaredServiceConfig",
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
class DeclaredServiceConfig:
    name: str = ""
    env_var: str = ""
    upstream: str = ""
    block_direct_egress: bool = False
    llm_dialect: str = ""
    dlp_redact_prompts: bool = False
    inspect_sse_tool_use: bool = False


@dataclass
class L7HttpConfig:
    sinkhole: SinkholeConfig = field(default_factory=lambda: SinkholeConfig())
    protocol_inspection: bool = False
    declared_services: list[DeclaredServiceConfig] = field(default_factory=list)
    threat_feeds: list[str] = field(default_factory=list)
    hash_only_llm_retention: bool = False


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
