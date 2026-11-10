# see: cans/artifacts/test-plan/cases/bindings_py/bp_01_python_boot_econfig_missing_pydantic_dependency.yaml#case_id
from __future__ import annotations

import asyncio

from kelir import KelirBuilder
from kelir.sdk_errors import EConfig


def bp_01_python_boot_econfig_missing_pydantic_dependency() -> None:
    try:
        asyncio.run(KelirBuilder().boot())
    except EConfig as exc:
        if "pydantic" not in str(exc).lower():
            raise AssertionError(
                "bp_01: builder.boot() must raise EConfig naming the missing "
                f"pydantic v2 dependency; got: {exc}"
            ) from exc
        return
    raise AssertionError(
        "bp_01: builder.boot() completed without the EConfig prerequisite gate"
    )


def test_bp_01_red() -> None:
    bp_01_python_boot_econfig_missing_pydantic_dependency()
