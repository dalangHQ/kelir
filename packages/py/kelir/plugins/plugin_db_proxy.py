# see: cans/plugins/db-proxy.md#Plugin-db-proxy
from __future__ import annotations

from dataclasses import dataclass, field

from kelir.plugin_model import BasePlugin

__all__ = [
    "DbProxyConfig",
    "DbProxyPlugin",
    "EphemeralCredentials",
    "PgWireRewrite",
    "SqliteOverVsock",
]


@dataclass
class PgWireRewrite:
    listen_port: int = 5432
    upstream: str = ""
    rewrite_rules: list[tuple[str, str]] = field(default_factory=list)
    max_rewrites_per_conn: int = 3


@dataclass
class SqliteOverVsock:
    guest_socket: str = "/run/kelir/db.sock"
    host_path: str = ""


@dataclass
class EphemeralCredentials:
    placeholder: str = ""
    ttl_seconds: int = 0


@dataclass
class DbProxyConfig:
    postgres: PgWireRewrite = field(default_factory=lambda: PgWireRewrite())
    sqlite: SqliteOverVsock = field(default_factory=lambda: SqliteOverVsock())
    credentials: EphemeralCredentials | None = None


class DbProxyPlugin(BasePlugin[None, None]):
    name = "db-proxy"

    def __init__(self, config: DbProxyConfig | None = None) -> None:
        self.config = config if config is not None else DbProxyConfig()


# see: cans/02-plugin-model.md#Unified-builder 
from kelir import KelirBuilder


def db_proxy_builder(config: DbProxyConfig | None = None) -> KelirBuilder:
    cfg = config if config is not None else DbProxyConfig()
    return KelirBuilder.plugin("db-proxy").context(lambda: cfg)


# see: cans/02-plugin-model.md#Unified-builder
def db_proxy(config: DbProxyConfig | None = None) -> KelirBuilder:
    return db_proxy_builder(config)
