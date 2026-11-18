# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

from dataclasses import dataclass, field

__all__ = ["IdmapConfig", "SharedCacheConfig", "StoragePlaneConfig"]


@dataclass
class IdmapConfig:
    guest_uid: int = 0
    host_uid: int = 0


# see: cans/05-storage-plane.md#Storage-architecture
@dataclass
class SharedCacheConfig:
    host_path: str = ""
    guest_mount: str = ""
    read_only: bool = False


# see: cans/05-storage-plane.md#Storage-architecture
@dataclass
class StoragePlaneConfig:
    idmap: IdmapConfig = field(default_factory=IdmapConfig)
    shared_caches: list[SharedCacheConfig] = field(default_factory=list)
    lock_broker: bool = False
    encapsulation: str = "Locked"
    dax_window: str | None = None
