# see: cans/03-sdk-architecture.md#Prerequisite-gating-across-languages
# see: cans/03-sdk-architecture.md#Distribution-and-error-invariants
from __future__ import annotations

import json
from typing import Any

ECONFIG_CODE = "EConfig"


def _optional_str(value: Any) -> str | None:
    return value if isinstance(value, str) else None


def parse_error_envelope(message: str) -> dict[str, Any] | None:
    try:
        parsed: Any = json.loads(message)
    except json.JSONDecodeError:
        return None
    if not isinstance(parsed, dict):
        return None
    if not isinstance(parsed.get("code"), str) or not isinstance(parsed.get("message"), str):
        return None
    return parsed


class EConfig(Exception):
    code: str
    driver: str | None
    ring: str | None
    pid: int | None
    binary: str | None
    errno: str | None

    def __init__(self, message: str, envelope: dict[str, Any] | None = None) -> None:
        super().__init__(message)
        fields = envelope or {}
        self.code = _optional_str(fields.get("code")) or ECONFIG_CODE
        self.driver = _optional_str(fields.get("driver"))
        self.ring = _optional_str(fields.get("ring"))
        pid = fields.get("pid")
        self.pid = pid if isinstance(pid, int) and not isinstance(pid, bool) else None
        self.binary = _optional_str(fields.get("binary"))
        self.errno = _optional_str(fields.get("errno"))

    @classmethod
    def from_native(cls, error: BaseException) -> EConfig:
        raw = str(error)
        envelope = parse_error_envelope(raw)
        if envelope is None:
            return cls(raw)
        return cls(str(envelope["message"]), envelope)
