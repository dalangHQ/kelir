# see: cans/artifacts/test-plan/cases/bindings_py/bp_03_python_fused_trampoline_single_crossing_chained_hooks.yaml#case_id
from __future__ import annotations

import asyncio
from typing import Any

from kelir import KelirBuilder
from kelir.plugin_fusion import RingEvent
from kelir.sdk_errors import EConfig


async def _noop(event: Any) -> None:
    return None


def bp_03_python_fused_trampoline_single_crossing_chained_hooks() -> None:
    builder = KelirBuilder()
    for _ in range(5):
        builder.trap("fs.write:*", pre=_noop)
    fused = builder.fuse()
    assert fused.crossings_per_ring_event(RingEvent.TRAP) == 1, (
        "bp_03: five fused Trap hooks do not collapse to exactly 1 crossing per ring event"
    )
    assert fused.hook_count(RingEvent.TRAP) == 5, (
        f"bp_03: the fused Trap ring carries {fused.hook_count(RingEvent.TRAP)} hooks, expected 5"
    )
    try:
        asyncio.run(builder.boot())
    except EConfig as exc:
        raise AssertionError(
            f"bp_03: the fused trampoline table never bound at boot: {exc}"
        ) from exc


def test_bp_03_red() -> None:
    bp_03_python_fused_trampoline_single_crossing_chained_hooks()
