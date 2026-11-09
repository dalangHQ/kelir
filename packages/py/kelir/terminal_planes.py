# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Callable

__all__ = ["PromptRule", "TokenShield"]


@dataclass
class TokenShield:
    max_bytes_total: int = 0
    max_lines_total: int = 0
    clamp_strategy: str = ""
    notice: str = ""


@dataclass
class PromptRule:
    pattern: Any = None
    timeout: float = 0.0
    verify_foreground: bool = False
    action: Callable[..., Any] | None = None
