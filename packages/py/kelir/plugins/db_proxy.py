# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

import struct
from dataclasses import dataclass

from kelir.generated.attest_telemetry import ExecutionReceipt
from kelir.plugins.plugin_db_proxy import *  # noqa: F401,F403
from kelir.plugins.plugin_db_proxy import __all__ as __all__  # noqa: F401

__all__ = [*__all__, "NetL7SqlExt", "net_l7_sql"]


# see: cans/plugins/db-proxy.md#SQL-receipt-extension
@dataclass(frozen=True)
class NetL7SqlExt:
    statements: int = 0
    distinct_shapes: int = 0


# see: cans/plugins/db-proxy.md#SQL-receipt-extension
def net_l7_sql(receipt: ExecutionReceipt) -> NetL7SqlExt | None:
    for extension in receipt.extensions:
        if extension.namespace != "net:l7:sql":
            continue
        if len(extension.payload) != 8:
            return None
        statements, distinct_shapes = struct.unpack("<II", extension.payload)
        return NetL7SqlExt(statements=statements, distinct_shapes=distinct_shapes)
    return None
