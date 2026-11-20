# see: cans/artifacts/test-plan/cases/bindings_py/bp_06_python_export_spec_renders_toml_fields.yaml#case_id
# see: cans/artifacts/test-plan/cases/bindings_py/bp_07_python_export_spec_round_trips_fixture_sections.yaml#case_id
from __future__ import annotations

import asyncio
from pathlib import Path

import pytest

from kelir import Sandbox


def bp_06_python_export_spec_renders_toml_fields(tmp_path: Path) -> None:
    profile = {"session": {"id": "s1", "compute": {"vcpus": 4, "memory_mb": 2048}}}
    sandbox = Sandbox.from_profile(profile, "profile.toml")
    out = tmp_path / "export.toml"
    asyncio.run(sandbox.export_spec(str(out)))
    assert out.read_text(encoding="utf-8") == (
        '[session]\nid = "s1"\n\n[session.compute]\nvcpus = 4\nmemory_mb = 2048\n'
    )


def test_bp_06_red(tmp_path: Path) -> None:
    bp_06_python_export_spec_renders_toml_fields(tmp_path)


# see: cans/plugins/profile.md#Bidirectional-extraction
def bp_07_python_export_spec_round_trips_fixture_sections(tmp_path: Path) -> None:
    yaml_module = pytest.importorskip("yaml")
    fixture = (
        Path(__file__).resolve().parents[4]
        / "cans"
        / "artifacts"
        / "examples"
        / "fixtures"
        / "kelir.yaml"
    )
    loaded = yaml_module.safe_load(fixture.read_text(encoding="utf-8"))
    sandbox = Sandbox.from_profile(loaded, str(fixture))
    out = tmp_path / "export.yaml"
    asyncio.run(sandbox.export_spec(str(out)))
    exported = yaml_module.safe_load(out.read_text(encoding="utf-8"))
    assert list(exported.keys()) == list(loaded.keys())
    assert exported == loaded


def test_bp_07_red(tmp_path: Path) -> None:
    bp_07_python_export_spec_round_trips_fixture_sections(tmp_path)
