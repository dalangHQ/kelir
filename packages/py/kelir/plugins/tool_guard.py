# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

from dataclasses import dataclass, field

from kelir.plugin import BasePlugin

__all__ = ["ApprovalEntry", "ToolGuardConfig", "ToolGuardPlugin"]


@dataclass
class ApprovalEntry:
    action: str = ""
    duration_ms: int = 0


@dataclass
class ToolGuardConfig:
    pin_schemas: bool = False
    max_invocations_per_trace: int = 0
    approvals: dict[str, ApprovalEntry] = field(default_factory=dict)


class ToolGuardPlugin(BasePlugin):
    name = "tool-guard"

    def __init__(self, config: ToolGuardConfig | None = None) -> None:
        self.config = config if config is not None else ToolGuardConfig()
