# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Literal

__all__ = ["CredentialDescriptor", "CredentialKind", "L4Proxy", "Mtls", "SinkholeConfig"]

CredentialKind = Literal["bearer_token", "sigv4", "mtls_bundle"]


@dataclass
class SinkholeConfig:
    allowlist: list[str] = field(default_factory=list)
    drop_all: bool = False


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
