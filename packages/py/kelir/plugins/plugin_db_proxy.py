# see: cans/plugins/db-proxy.md#Plugin-db-proxy
from __future__ import annotations

from dataclasses import dataclass, field

from kelir.model_plugin import BasePlugin

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


class DbProxyPlugin(BasePlugin):
    name = "db-proxy"

    def __init__(self, config: DbProxyConfig | None = None) -> None:
        self.config = config if config is not None else DbProxyConfig()


# see: cans/02-plugin-model.md#Fluent-plugin-creation-builder 
from kelir.plugin_builder import PluginBuilder, _PluginWrapper


def db_proxy_builder(config: DbProxyConfig | None = None) -> _PluginWrapper:
    cfg = config if config is not None else DbProxyConfig()
    return PluginBuilder("db-proxy").context(lambda: cfg).build()
