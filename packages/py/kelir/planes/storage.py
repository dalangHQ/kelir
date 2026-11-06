# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Callable

__all__ = ["DynamicFile", "IdmapConfig", "MaskRule", "SnoopRule", "SyntheticFile"]


@dataclass
class DynamicFile:
    content: str = ""
    mode: int = 0o400


@dataclass
class SyntheticFile:
    path: str = ""
    content: str = ""
    dynamic_fn: Callable[..., Any] | None = None

    @classmethod
    def static(cls, path: str, content: str) -> SyntheticFile:
        return cls(path=path, content=content)

    @classmethod
    def dynamic(cls, path: str, fn: Callable[..., Any]) -> SyntheticFile:
        return cls(path=path, dynamic_fn=fn)


@dataclass
class IdmapConfig:
    guest_uid: int = 0
    host_uid: int = 0


@dataclass
class MaskRule:
    glob: str = ""
    patterns: list[tuple[str, str]] = field(default_factory=list)


@dataclass
class SnoopRule:
    glob: str = ""
    sync: bool = False
    on_write: Callable[..., Any] | None = None
