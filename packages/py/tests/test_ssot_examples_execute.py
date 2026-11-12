# see: cans/02-plugin-model.md#Unified-builder
from __future__ import annotations

import asyncio
import importlib.util
import sys
from pathlib import Path
from types import SimpleNamespace
from typing import Any

import pytest

from kelir import KelirBuilder

_EXAMPLES = Path(__file__).resolve().parents[3] / "cans" / "artifacts" / "examples"


def _load_example(name: str) -> Any:
    path = _EXAMPLES / f"{name}.py"
    module_name = f"kelir_example_{name.replace('-', '_')}"
    spec = importlib.util.spec_from_file_location(module_name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    dont_write = sys.dont_write_bytecode
    sys.dont_write_bytecode = True
    try:
        spec.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = dont_write
    return module


# TODO (L3) [bindings_py]: the stub below stands in for the guest boot only; the builder chain and main() logic run for real, the microVM does not boot on this host.; slop: stubbed boot hides real boot failures
class _StubChannel:
    async def write(self, data: bytes) -> None:
        return None

    async def read(self) -> bytes:
        return b"policy-matrix:pong"


class _StubVsock:
    async def channel(self, kind: int) -> _StubChannel:
        return _StubChannel()


class _StubSession:
    driver_name = "stub"
    boot_ms = 0
    vsock = _StubVsock()

    async def spawn(self, *args: Any, **kwargs: Any) -> SimpleNamespace:
        return SimpleNamespace(exit_code=0)

    async def suspend(self, *args: Any, **kwargs: Any) -> None:
        return None

    async def resume(self, *args: Any, **kwargs: Any) -> None:
        return None


async def _stub_aenter(self: KelirBuilder) -> _StubSession:
    return _StubSession()


async def _stub_aexit(self: KelirBuilder, *args: Any) -> None:
    return None


def _stub_boot(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(KelirBuilder, "__aenter__", _stub_aenter)
    monkeypatch.setattr(KelirBuilder, "__aexit__", _stub_aexit)


def test_policy_matrix_build_runtime_executes() -> None:
    module = _load_example("policy-matrix")
    builder = module.build_runtime()
    assert isinstance(builder, KelirBuilder)
    assert isinstance(builder.fuse().describe(), dict)


def test_policy_matrix_main_executes_with_stubbed_boot(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    _stub_boot(monkeypatch)
    module = _load_example("policy-matrix")
    asyncio.run(module.main())


def test_db_backed_runtime_build_runtime_executes() -> None:
    module = _load_example("db-backed-runtime")
    builder = module.build_runtime("127.0.0.1", "secret")
    assert isinstance(builder, KelirBuilder)
    assert isinstance(builder.fuse().describe(), dict)


def test_container_build_runtime_executes() -> None:
    module = _load_example("container")
    builder = module.build_runtime()
    assert isinstance(builder, KelirBuilder)
    assert isinstance(builder.fuse().describe(), dict)


def test_autonomous_swe_agent_build_runtime_executes() -> None:
    module = _load_example("autonomous-swe-agent")
    builder = asyncio.run(
        module.build_runtime(
            oracle=module.UserOracle(),
            pg_cert=Path("cert.pem"),
            pg_key=Path("key.pem"),
        )
    )
    assert isinstance(builder, KelirBuilder)
    assert isinstance(builder.fuse().describe(), dict)


def test_hook_matrix_main_executes_with_stubbed_boot(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    _stub_boot(monkeypatch)
    module = _load_example("hook-matrix")
    asyncio.run(module.main())
