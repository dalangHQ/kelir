# see: cans/03-sdk-architecture.md#Session-surfaces
from __future__ import annotations

from typing import Any

from kelir.sdk_errors import EConfig


def _missing(what: str) -> EConfig:
    return EConfig(f"kelir native PyO3 bridge is missing: {what} (L3)")


# TODO (L3) [protocol_mux]: exec_in_guest rides the EXEC stream kind via the shim wire protocol.; slop: exec_in_guest always raises, no guest command path exists.
async def lifecycle_exec_in_guest(argv: list[str]) -> Any:
    void = argv
    raise _missing("lifecycle exec_in_guest")
