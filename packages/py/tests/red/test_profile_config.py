# see: cans/artifacts/test-plan/cases/bindings_py/bp_05_python_from_config_loads_toml.yaml#case_id
from __future__ import annotations

import asyncio
from pathlib import Path

from kelir import Sandbox


def bp_05_python_from_config_loads_toml(tmp_path: Path) -> None:
    profile = tmp_path / "profile.toml"
    profile.write_text(
        '[session]\nid = "sess-toml"\n\n[session.compute]\nvcpus = 2\nmemory_mb = 2048\n',
        encoding="utf-8",
    )
    sandbox = asyncio.run(Sandbox.from_config(str(profile)))
    assert sandbox.session_id() == "sess-toml"
    assert sandbox.source_path == str(profile)


def test_bp_05_red(tmp_path: Path) -> None:
    bp_05_python_from_config_loads_toml(tmp_path)
