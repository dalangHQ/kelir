# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable

from kelir.sdk_errors_flow import EConfig

__all__ = [
    "L4Proxy",
    "MtlsConfig",
    "PtyConfig",
    "SinkholeConfig",
    "SinkholeRule",
    "TerminalSanitizerConfig",
    "TerminalWireframeGridConfig",
    "WireConfig",
]


# see: cans/07-wire-plane.md#Sinkhole-rules
@dataclass
class SinkholeRule:
    host: str = ""
    allow: bool = False


# see: cans/07-wire-plane.md#Sinkhole-rules
@dataclass
class SinkholeConfig:
    rules: list[SinkholeRule] = field(default_factory=list)


@dataclass
class MtlsConfig:
    cert_path: Path | str | None = None
    key_path: Path | str | None = None


# see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
@dataclass
class L4Proxy:
    listen_port: int = 0
    destination: str = ""
    forward_unix_socket: Path | str | None = None
    inject_mtls: MtlsConfig | None = None


# see: cans/07-wire-plane.md#Wire-plane
@dataclass
class WireConfig:
    userspace_netstack: bool | None = None
    default_egress: str | None = None
    dial_timeout_ms: int = 5000
    block_abstract_unix_sockets: bool | None = None
    divert_ports: list[tuple[int, str]] = field(default_factory=list)

    def divert_port_to_unix(self, port: int, socket_path: str) -> None:
        if isinstance(port, bool) or not isinstance(port, int) or not 1 <= port <= 65535:
            raise EConfig(f"divert_port_to_unix port {port!r} outside 1..65535")
        if not socket_path:
            raise EConfig("divert_port_to_unix expects a non-empty unix socket path")
        self.divert_ports.append((port, socket_path))

    def to_wire_dict(self) -> dict[str, Any]:
        merged: dict[str, Any] = {"dial_timeout_ms": self.dial_timeout_ms}
        if self.userspace_netstack is not None:
            merged["userspace_netstack"] = self.userspace_netstack
        if self.default_egress is not None:
            merged["default_egress"] = self.default_egress
        if self.block_abstract_unix_sockets is not None:
            merged["block_abstract_unix_sockets"] = self.block_abstract_unix_sockets
        if self.divert_ports:
            merged["divert_ports"] = [
                {"listen_port": port, "forward_unix_socket": socket_path}
                for port, socket_path in self.divert_ports
            ]
        return merged


# see: cans/04-terminal-plane.md#Headless-VT100-wireframe-grid
@dataclass
class TerminalWireframeGridConfig:
    track_screen_matrix: bool = False
    export_on_trap: bool = False


# see: cans/04-terminal-plane.md#Terminal-pipeline
@dataclass
class TerminalSanitizerConfig:
    drop_osc52: bool = False
    strip_title: bool = False


# see: cans/04-terminal-plane.md#Terminal-pipeline
# see: cans/artifacts/examples/fixtures/kelir.yaml#terminal
@dataclass
class PtyConfig:
    cols: int = 0
    rows: int = 0
    wireframe_grid: TerminalWireframeGridConfig = field(
        default_factory=TerminalWireframeGridConfig
    )
    # see: cans/artifacts/examples/fixtures/kelir.yaml#terminal.stream_engine
    stream_engine: str = ""
    sanitizer: TerminalSanitizerConfig = field(
        default_factory=TerminalSanitizerConfig
    )


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
        dial_timeout_ms=wire.get("dial_timeout_ms", 5000),
        divert_ports=[
            pair for pair in (_divert_pair(entry) for entry in wire.get("divert_ports", []))
            if pair is not None
        ],
    )
    fn(config)
    merged = dict(wire)
    merged.update(config.to_wire_dict())
    return merged
