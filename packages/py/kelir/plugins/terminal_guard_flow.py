# see: cans/plugins/terminal-guard.md#Plugin-terminal-guard
from __future__ import annotations

from dataclasses import dataclass, field

from kelir.plugin_model_flow import BasePlugin

__all__ = [
    "PromptRuleConfig",
    "TerminalGuardConfig",
    "TerminalGuardPlugin",
    "terminal_guard",
    "terminal_guard_builder",
]


@dataclass
class PromptRuleConfig:
    pattern: str = ""
    timeout_seconds: int = 0
    verify_foreground: bool = False
    action: list[int] = field(default_factory=list)


@dataclass
class TerminalGuardConfig:
    prompts: list[PromptRuleConfig] = field(default_factory=list)
    notice_template: str = ""


class TerminalGuardPlugin(BasePlugin[None, None]):
    name = "terminal-guard"

    def __init__(self, config: TerminalGuardConfig | None = None) -> None:
        self.config = config if config is not None else TerminalGuardConfig()


# see: cans/02-plugin-model.md#Unified-builder 
from kelir import KelirBuilder


def terminal_guard_builder(config: TerminalGuardConfig | None = None) -> KelirBuilder:
    cfg = config if config is not None else TerminalGuardConfig()
    return KelirBuilder.plugin("terminal-guard").context(lambda: cfg)


# see: cans/02-plugin-model.md#Unified-builder
def terminal_guard(config: TerminalGuardConfig | None = None) -> KelirBuilder:
    return terminal_guard_builder(config)
