# see: cans/plugins/db-proxy.md#Plugin-db-proxy
from __future__ import annotations

from dataclasses import dataclass, field

from kelir.plugin_model_flow import BasePlugin

__all__ = [
    "DbProxyConfig",
    "DbProxyPlugin",
    "EphemeralCredentials",
    "PgSchemaRewrite",
    "PgWireRewrite",
    "SqliteOverVsock",
    "db_proxy",
    "db_proxy_builder",
]


@dataclass
class PgSchemaRewrite:
    from_schema: str = ""
    to_schema: str = ""


@dataclass
class PgWireRewrite:
    listen_port: int = 5432
    upstream: str = ""
    schema_rewrites: list[PgSchemaRewrite] = field(default_factory=list)
    max_rewrites_per_conn: int = 3
    ast_parser: str = "libpgquery"
    require_where_on_mutations: bool = True
    deny_mode_in_tx: str = "terminate"
    block_unsafe_io: bool = True
    catalog_resolution: bool = True
    synthetic_cancel_keys: bool = True


@dataclass
class SqliteOverVsock:
    guest_socket: str = "/run/kelir/db.sock"
    host_path: str = ""


@dataclass
class EphemeralCredentials:
    placeholder: str = ""
    ttl_seconds: int = 60


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
