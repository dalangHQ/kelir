# see: cans/02-plugin-model.md#Unified-builder
# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import pytest

_BAD_CASES: list[tuple[str, str, str]] = [
    (
        "scope_bleed",
        'KelirBuilder.plugin("x").fs(lambda f: f.on_trap(lambda e: Verdict.pass_()))',
        'Cannot access attribute "on_trap" for class "FsScopeProtocol"',
    ),
    (
        "host_verb_on_plugin",
        'KelirBuilder.plugin("x").storage(a=1)',
        'Cannot access attribute "storage"',
    ),
    (
        "context_on_host",
        "KelirBuilder().context(lambda: {})",
        'Cannot access attribute "context"',
    ),
    (
        "post_seal_storage",
        "KelirBuilder().seal().storage(a=1)",
        'Cannot access attribute "storage"',
    ),
    (
        "deleted_flat_verb",
        'KelirBuilder.plugin("x").on_fs_write(lambda p, d, pid, c: FilterAction.forward())',
        'Cannot access attribute "on_fs_write"',
    ),
    (
        "boot_on_plugin",
        'KelirBuilder.plugin("x").boot()',
        'Cannot access attribute "boot"',
    ),
    (
        "direct_model_params",
        "KelirBuilder().params(dict)",
        'cannot be assigned to parameter "fn"',
    ),
    (
        "post_seal_trap",
        'KelirBuilder().seal().trap(lambda t: t.on_trap(lambda e: Verdict.pass_()))',
        'Cannot access attribute "trap"',
    ),
]

_BAD_TEMPLATE = """from kelir import KelirBuilder, Verdict, FilterAction


{body}
"""

_GOOD_TEMPLATE = """from kelir import KelirBuilder, Verdict, FilterAction


def good_chain() -> None:
    plugin = (
        KelirBuilder.plugin("x")
        .trap(lambda t: t.on_trap(lambda e: Verdict.pass_()))
        .fs(lambda f: f.on_fs_write(lambda p, d, pid, c: FilterAction.forward()))
    )
    host = KelirBuilder().use(plugin).seal()
    assert host is not None
"""


def _pyright_json(path: Path) -> dict[str, object]:
    try:
        proc = subprocess.run(
            [
                sys.executable,
                "-m",
                "pyright",
                "--outputjson",
                "--pythonpath",
                sys.executable,
                str(path),
            ],
            capture_output=True,
            text=True,
            timeout=120,
        )
    except FileNotFoundError as exc:
        pytest.fail(f"pyright is not runnable: {exc} (run: uv pip install -e \"packages/py[dev]\")")
    try:
        return json.loads(proc.stdout)  # type: ignore[return-value]
    except json.JSONDecodeError:
        pytest.fail(f"pyright produced no JSON output: rc={proc.returncode} stderr={proc.stderr[:500]}")


def test_compile_fail_cases(tmp_path: Path) -> None:
    body = "\n\n".join(
        f"def case_{name}() -> None:\n    {snippet}" for name, snippet, _ in _BAD_CASES
    )
    bad_path = tmp_path / "bad.py"
    bad_path.write_text(_BAD_TEMPLATE.format(body=body))
    result = _pyright_json(bad_path)
    diagnostics = result.get("generalDiagnostics", [])
    assert isinstance(diagnostics, list)
    lines = bad_path.read_text().splitlines()
    for name, snippet, expected in _BAD_CASES:
        call_line = next(
            i + 1 for i, line in enumerate(lines) if snippet in line
        )
        matches = [
            d
            for d in diagnostics
            if d["range"]["start"]["line"] + 1 == call_line
            and expected in d["message"]
        ]
        assert matches, (
            f"pyright reported no '{expected}' diagnostic on line {call_line} "
            f"for case {name}; got: {[d['message'] for d in diagnostics]}"
        )
    assert len(diagnostics) == len(_BAD_CASES), (
        f"pyright reported {len(diagnostics)} diagnostics, want exactly "
        f"{len(_BAD_CASES)} (one per case); got: "
        f"{[d['message'] for d in diagnostics]}"
    )


def test_compile_pass_control(tmp_path: Path) -> None:
    good_path = tmp_path / "good.py"
    good_path.write_text(_GOOD_TEMPLATE)
    result = _pyright_json(good_path)
    diagnostics = result.get("generalDiagnostics", [])
    assert diagnostics == [], f"pyright reported diagnostics on the control file: {diagnostics}"
