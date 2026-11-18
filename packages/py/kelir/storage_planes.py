# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

from dataclasses import dataclass

__all__ = ["IdmapConfig"]


@dataclass
class IdmapConfig:
    guest_uid: int = 0
    host_uid: int = 0
