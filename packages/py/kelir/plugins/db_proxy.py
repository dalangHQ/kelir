# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Callable

from kelir.plugin import BasePlugin

__all__ = [
    "DbProxyConfig",
    "DbProxyPlugin",
    "EphemeralCredentials",
    "PgWireRewrite",
    "SqliteOverVsock",
]


@dataclass
class PgWireRewrite:
    listen_port: int = 0
    upstream: str = ""
    rewrite_rules: list[tuple[str, str]] = field(default_factory=list)
    max_rewrites_per_conn: int = 0


@dataclass
class EphemeralCredentials:
    placeholder: str = ""
    resolve: Callable[[], str] | None = None
    ttl_seconds: int = 0


@dataclass
class SqliteOverVsock:
    guest_socket: str = ""
    host_path: str = ""


@dataclass
class DbProxyConfig:
    postgres: PgWireRewrite | None = None
    credentials: EphemeralCredentials | None = None
    sqlite: SqliteOverVsock | None = None


class DbProxyPlugin(BasePlugin):
    name = "db-proxy"

    def __init__(self, config: DbProxyConfig | None = None) -> None:
        self.config = config if config is not None else DbProxyConfig()
