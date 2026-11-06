# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

__all__ = ["L4Proxy", "Mtls", "SinkholeConfig", "VaultEntry"]


@dataclass
class SinkholeConfig:
    allowlist: list[str] = field(default_factory=list)
    drop_all: bool = False


@dataclass
class VaultEntry:
    name: str = ""
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
