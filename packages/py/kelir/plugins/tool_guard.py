# see: cans/plugins/tool-guard.md#Plugin-tool-guard
from __future__ import annotations

from dataclasses import dataclass, field

from kelir.plugin_model import BasePlugin

__all__ = [
    "ApprovalAuthConfig",
    "ToolGuardConfig",
    "ToolGuardPlugin",
    "tool_guard",
    "tool_guard_builder",
]


@dataclass
class ApprovalAuthConfig:
    deny_self_approval: bool = True
    methods: list[str] = field(default_factory=lambda: ["webauthn", "totp", "oidc"])
    max_pending_holds: int = 10
    hold_ttl_seconds: int = 300


@dataclass
class ToolGuardConfig:
    max_invocations_per_trace: int = 50
    pin_schemas: bool = True
    max_frame_bytes: int = 16_777_216
    detect_semantic_rug_pull: bool = True
    strict_ast_parameter_validation: bool = True
    propagate_byte_range_taints: bool = True
    detect_cross_server_exfil: bool = True
    detect_confusable_names: bool = True
    scan_tool_results: bool = True
    approval_auth: ApprovalAuthConfig = field(default_factory=lambda: ApprovalAuthConfig())


class ToolGuardPlugin(BasePlugin[None, None]):
    name = "tool-guard"

    def __init__(self, config: ToolGuardConfig | None = None) -> None:
        self.config = config if config is not None else ToolGuardConfig()


# see: cans/02-plugin-model.md#Unified-builder 
from kelir import KelirBuilder


def tool_guard_builder(config: ToolGuardConfig | None = None) -> KelirBuilder:
    cfg = config if config is not None else ToolGuardConfig()
    return KelirBuilder.plugin("tool-guard").require("mcp").context(lambda: cfg)


# see: cans/02-plugin-model.md#Unified-builder
def tool_guard(config: ToolGuardConfig | None = None) -> KelirBuilder:
    return tool_guard_builder(config)
