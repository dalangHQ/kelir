# see: cans/plugins/step-eval.md#Plugin-step-eval
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Literal

from kelir.plugin_model import BasePlugin

__all__ = [
    "StepDagConfig",
    "StepEvalAssertions",
    "StepEvalConfig",
    "StepEvalPlugin",
    "StepEvaluationConfig",
    "step_eval",
    "step_eval_builder",
]


@dataclass
class StepEvalAssertions:
    assert_binary_blake3: bool = True
    max_egress_bytes_hermetic: int = 0
    require_non_zero_delta: bool = True
    max_duration_seconds: int = 120


@dataclass
class StepEvaluationConfig:
    enabled: bool = True
    default_action: Literal["proceed", "rollback-and-halt"] = "proceed"
    assertions: StepEvalAssertions = field(default_factory=lambda: StepEvalAssertions())


@dataclass
class StepDagConfig:
    enabled: bool = True
    require_signed_attribution: bool = True
    require_separation_of_duties: bool = True
    storage: Literal["content-addressed"] = "content-addressed"


@dataclass
class StepEvalConfig:
    sentinel_path: str = "/dev/kelir-gate"
    pty_quiescence: bool = True
    step_dag: StepDagConfig = field(default_factory=lambda: StepDagConfig())
    step_evaluation: StepEvaluationConfig = field(default_factory=lambda: StepEvaluationConfig())


class StepEvalPlugin(BasePlugin[None, None]):
    name = "step-eval"

    def __init__(self, config: StepEvalConfig | None = None) -> None:
        self.config = config if config is not None else StepEvalConfig()


# see: cans/02-plugin-model.md#Unified-builder 
from kelir import KelirBuilder


def step_eval_builder(config: StepEvalConfig | None = None) -> KelirBuilder:
    cfg = config if config is not None else StepEvalConfig()
    return KelirBuilder.plugin("step-eval").context(lambda: cfg)


# see: cans/02-plugin-model.md#Unified-builder
def step_eval(config: StepEvalConfig | None = None) -> KelirBuilder:
    return step_eval_builder(config)
