# see: cans/plugins/db-proxy.md#Plugin-db-proxy
from __future__ import annotations

import struct
from dataclasses import dataclass, field

from kelir.generated.attest_telemetry import ExecutionReceipt
from kelir.plugin_model import BasePlugin

__all__ = [
    "DbProxyConfig",
    "DbProxyPlugin",
    "EphemeralCredentials",
    "NetL7SqlExt",
    "PgSchemaRewrite",
    "PgWireRewrite",
    "SqliteOverVsock",
    "db_proxy",
    "db_proxy_builder",
    "net_l7_sql",
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



# see: cans/plugins/db-proxy.md#SQL-receipt-extension
@dataclass(frozen=True)
class NetL7SqlExt:
    statements: int = 0
    distinct_shapes: int = 0


# see: cans/plugins/db-proxy.md#SQL-receipt-extension
def net_l7_sql(receipt: ExecutionReceipt) -> NetL7SqlExt | None:
    for extension in receipt.extensions:
        if extension.namespace != "net:l7:sql":
            continue
        if len(extension.payload) != 8:
            return None
        statements, distinct_shapes = struct.unpack("<II", extension.payload)
        return NetL7SqlExt(statements=statements, distinct_shapes=distinct_shapes)
    return None
