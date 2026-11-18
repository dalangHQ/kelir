# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable, Literal

from kelir.sdk_errors import EConfig

__all__ = [
    "CredentialDescriptor",
    "CredentialKind",
    "L4Proxy",
    "Mtls",
    "SinkholeConfig",
    "SinkholeRule",
    "WireConfig",
]

CredentialKind = Literal["bearer_token", "sigv4", "mtls_bundle"]


# see: cans/07-wire-plane.md#Sinkhole-rules
@dataclass
class SinkholeRule:
    host: str = ""
    methods: list[str] | None = None
    paths: list[str] | None = None
    allow: bool = False


# see: cans/07-wire-plane.md#Sinkhole-rules
@dataclass
class SinkholeConfig:
    rules: list[SinkholeRule] = field(default_factory=list)

    @classmethod
    def from_legacy(cls, allowlist: list[str], drop_all: bool) -> SinkholeConfig:
        # see: cans/07-wire-plane.md#Sinkhole-rules
        rules = [SinkholeRule(host=host, allow=True) for host in allowlist]
        if drop_all:
            rules.append(SinkholeRule(host="*", allow=False))
        return cls(rules=rules)


@dataclass
class CredentialDescriptor:
    name: str = ""
    kind: CredentialKind = "bearer_token"
    audience: str = ""
    placeholder: str = ""


@dataclass
class Mtls:
    cert_path: Path | str | None = None
    key_path: Path | str | None = None


@dataclass
class L4Proxy:
    listen_port: int = 0
    destination: str = ""
    inject_mtls: Mtls | None = None


# see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
@dataclass
class WireConfig:
    userspace_netstack: bool | None = None
    default_egress: str | None = None
    divert_ports: list[tuple[int, str]] = field(default_factory=list)

    def divert_port_to_unix(self, port: int, socket_path: str) -> None:
        if isinstance(port, bool) or not isinstance(port, int) or not 1 <= port <= 65535:
            raise EConfig(f"divert_port_to_unix port {port!r} outside 1..65535")
        if not socket_path:
            raise EConfig("divert_port_to_unix expects a non-empty unix socket path")
        self.divert_ports.append((port, socket_path))

    def to_wire_dict(self) -> dict[str, Any]:
        merged: dict[str, Any] = {}
        if self.userspace_netstack is not None:
            merged["userspace_netstack"] = self.userspace_netstack
        if self.default_egress is not None:
            merged["default_egress"] = self.default_egress
        if self.divert_ports:
            merged["divert_ports"] = [
                {"listen_port": port, "forward_unix_socket": socket_path}
                for port, socket_path in self.divert_ports
            ]
        return merged


# see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
def _divert_pair(entry: Any) -> tuple[int, str] | None:
    if isinstance(entry, dict):
        port = entry.get("listen_port")
        socket_path = entry.get("forward_unix_socket")
        if isinstance(port, int) and isinstance(socket_path, str):
            return (port, socket_path)
        return None
    if isinstance(entry, (tuple, list)) and len(entry) == 2:
        port, socket_path = entry
        if isinstance(port, int) and isinstance(socket_path, str):
            return (port, socket_path)
    return None


# see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
def apply_wire_config(wire: dict[str, Any], fn: Callable[[WireConfig], Any]) -> dict[str, Any]:
    config = WireConfig(
        userspace_netstack=wire.get("userspace_netstack"),
        default_egress=wire.get("default_egress"),
        divert_ports=[
            pair for pair in (_divert_pair(entry) for entry in wire.get("divert_ports", []))
            if pair is not None
        ],
    )
    fn(config)
    merged = dict(wire)
    merged.update(config.to_wire_dict())
    return merged
