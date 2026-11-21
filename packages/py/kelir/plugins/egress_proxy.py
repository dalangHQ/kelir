# see: cans/plugins/egress-proxy.md#Plugin-egress-proxy
from __future__ import annotations

from dataclasses import dataclass, field

from kelir.plugin_model import BasePlugin

__all__ = [
    "EgressProxyConfig",
    "EgressProxyPlugin",
    "egress_proxy",
    "egress_proxy_builder",
]


@dataclass
class EgressProxyConfig:
    http_proxy: str | None = None
    https_proxy: str | None = None
    all_proxy: str | None = None
    no_proxy: list[str] = field(default_factory=list)
    block_tor: bool = True
    deny_socks5_resolve: bool = True


class EgressProxyPlugin(BasePlugin[None, None]):
    name = "egress-proxy"

    def __init__(self, config: EgressProxyConfig | None = None) -> None:
        self.config = config if config is not None else EgressProxyConfig()


# see: cans/02-plugin-model.md#Unified-builder 
from kelir import KelirBuilder


def egress_proxy_builder(config: EgressProxyConfig | None = None) -> KelirBuilder:
    cfg = config if config is not None else EgressProxyConfig()
    return KelirBuilder.plugin("egress-proxy").context(lambda: cfg)


# see: cans/02-plugin-model.md#Unified-builder
def egress_proxy(config: EgressProxyConfig | None = None) -> KelirBuilder:
    return egress_proxy_builder(config)
