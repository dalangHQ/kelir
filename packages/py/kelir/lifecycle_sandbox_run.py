# see: cans/plugins/profile.md#Declarative-profile-execution
from __future__ import annotations

import importlib
import tomllib
from pathlib import Path
from typing import Any

import tomli_w

from kelir.sdk_errors_flow import EConfig


# TODO (L3) [profile_ingestion]: read the profile file bytes and pick toml/yaml by extension.; slop: profile read never lands, from_config throws instead of loading
def _read_profile_text(origin: str, path: str) -> str:
    return Path(path).read_text(encoding="utf-8")


# TODO (L3) [profile_ingestion]: parse toml/yaml text into the profile mapping.; slop: profile parse never lands, from_config throws instead of loading
def _parse_profile_text(origin: str, path: str, text: str) -> dict[str, Any]:
    if path.endswith(".toml"):
        return tomllib.loads(text)
    if path.endswith((".yaml", ".yml")):
        yaml = importlib.import_module("yaml")
        return dict(yaml.safe_load(text))
    raise EConfig(
        f"{origin}: unsupported profile extension for `{path}` (expected .toml, .yaml, or .yml)"
    )


# TODO (L3) [profile_ingestion]: serialize the profile model back to toml/yaml text.; slop: profile text never lands, export_spec drops the profile
def _profile_text(model: Any, path: str) -> str:
    data = model.model_dump(exclude_none=True)
    if path.endswith((".yaml", ".yml")):
        yaml = importlib.import_module("yaml")
        return str(yaml.safe_dump(data, sort_keys=False))
    return tomli_w.dumps(data)


# TODO (L3) [profile_ingestion]: write the serialized profile text to the target path.; slop: profile write never lands, export_spec drops the profile
def _write_profile_text(path: str, text: str) -> None:
    Path(path).write_text(text, encoding="utf-8")
