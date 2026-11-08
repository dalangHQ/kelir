# see: cans/03-sdk-architecture.md#Sandbox-high-level-facade
from __future__ import annotations

import importlib
from pathlib import Path
from typing import TYPE_CHECKING, Any

from kelir.errors import EConfig as EConfig

if TYPE_CHECKING:
    from kelir import CommitStats, KelirBuilder, Session, SpawnResult


def _toml_scalar(value: Any) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, float):
        return repr(value)
    if isinstance(value, str):
        if "\n" in value:
            escaped = value.replace("\\", "\\\\").replace('"""', '\\"""')
            if value.startswith("\n"):
                escaped = "\\n" + escaped[1:]
            return '"""' + escaped + '"""'
        return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'
    if isinstance(value, list):
        return "[" + ", ".join(_toml_scalar(item) for item in value) + "]"
    raise EConfig(f"Sandbox.export_spec: unsupported TOML value {type(value).__name__}")


def _dump_toml_section(
    table: dict[str, Any], prefix: str, lines: list[str], array: bool = False
) -> None:
    scalars = {
        k: v
        for k, v in table.items()
        if not isinstance(v, dict)
        and not (isinstance(v, list) and v and isinstance(v[0], dict))
    }
    tables = {k: v for k, v in table.items() if isinstance(v, dict)}
    array_tables = {
        k: v
        for k, v in table.items()
        if isinstance(v, list) and v and isinstance(v[0], dict)
    }
    if prefix:
        lines.append(f"[[{prefix}]]" if array else f"[{prefix}]")
    for key, value in scalars.items():
        lines.append(f"{key} = {_toml_scalar(value)}")
    for key, value in tables.items():
        lines.append("")
        _dump_toml_section(value, f"{prefix}.{key}" if prefix else key, lines)
    for key, rows in array_tables.items():
        for row in rows:
            lines.append("")
            _dump_toml_section(
                row, f"{prefix}.{key}" if prefix else key, lines, array=True
            )


def _dump_toml(profile: dict[str, Any]) -> str:
    lines: list[str] = []
    _dump_toml_section(profile, "", lines)
    return "\n".join(lines) + "\n"


class Sandbox:
    def __init__(
        self,
        builder: KelirBuilder,
        profile: dict[str, Any],
        source_path: str,
    ) -> None:
        self._builder = builder
        self._session: Session | None = None
        self._profile = profile
        self._source_path = source_path

    @classmethod
    async def from_config(cls, path: str) -> Sandbox:
        text = Path(path).read_text(encoding="utf-8")
        if path.endswith(".toml"):
            import tomllib

            loaded = tomllib.loads(text)
            profile: dict[str, Any] = loaded if isinstance(loaded, dict) else {}
        elif path.endswith((".yaml", ".yml")):
            yaml = importlib.import_module("yaml")
            loaded = yaml.safe_load(text)
            profile = loaded if isinstance(loaded, dict) else {}
        else:
            raise EConfig(
                f"Sandbox.from_config: unsupported profile extension for `{path}` (expected .toml, .yaml, or .yml)"
            )
        return cls.from_profile(profile, path)

    @classmethod
    def from_profile(cls, profile: dict[str, Any], source_path: str) -> Sandbox:
        session = profile.get("session") or {}
        compute = session.get("compute") or {}
        artifacts = session.get("artifacts") or {}
        clock = session.get("clock") or {}
        watchdog = session.get("watchdog") or {}
        from kelir import KelirBuilder

        builder = KelirBuilder(
            vcpus=compute.get("vcpus"),
            memory_mb=compute.get("memory_mb"),
            kernel=artifacts.get("kernel_path"),
            rootfs_erofs=artifacts.get("rootfs_erofs_path"),
            tsc_locked=clock.get("tsc_locked"),
            enable_watchdog=watchdog.get("enabled"),
        )
        return cls(builder, profile, source_path)

    @classmethod
    async def create(
        cls,
        *,
        vcpus: int | None = None,
        memory_mb: int | None = None,
        kernel: str | None = None,
        rootfs_erofs: str | None = None,
        tsc_locked: bool | None = None,
        enable_watchdog: bool | None = None,
        driver: str | None = None,
        detached: bool = False,
    ) -> Sandbox:
        from kelir import KelirBuilder

        builder = KelirBuilder(
            vcpus=vcpus,
            memory_mb=memory_mb,
            kernel=kernel,
            rootfs_erofs=rootfs_erofs,
            tsc_locked=tsc_locked,
            enable_watchdog=enable_watchdog,
            driver=driver,
        ).detached(detached)
        return cls(builder, {}, "")

    @property
    def profile(self) -> dict[str, Any]:
        return self._profile

    @property
    def source_path(self) -> str:
        return self._source_path

    @property
    def session(self) -> Session | None:
        return self._session

    def session_id(self) -> str | None:
        session = self._profile.get("session") or {}
        value = session.get("id")
        return value if isinstance(value, str) else None

    def is_detached(self) -> bool:
        return self._builder.is_detached()

    async def boot(self) -> Sandbox:
        self._session = await self._builder.boot()
        return self

    async def exec(
        self, binary: str, argv: list[str] | None = None
    ) -> SpawnResult:
        if self._session is None:
            raise EConfig("Sandbox.exec before boot")
        return await self._session.spawn(binary, argv)

    async def rollback(self) -> None:
        if self._session is None:
            raise EConfig("Sandbox.rollback before boot")
        await self._session.rollback()

    async def suspend(self, path: str) -> None:
        if self._session is None:
            raise EConfig("Sandbox.suspend before boot")
        await self._session.suspend(path)

    async def resume(self, path: str) -> None:
        if self._session is None:
            raise EConfig("Sandbox.resume before boot")
        await self._session.resume(path)

    async def reconcile(
        self,
        cwd: str | None = None,
        atomic_staging: bool = False,
        fsync: bool = False,
    ) -> CommitStats:
        if self._session is None:
            raise EConfig("Sandbox.reconcile before boot")
        return await self._session.storage.reconcile(
            cwd=cwd, atomic_staging=atomic_staging, fsync=fsync
        )

    async def export_spec(self, path: str) -> None:
        Path(path).write_text(_dump_toml(self._profile), encoding="utf-8")

    async def stop(self) -> None:
        if self._session is None:
            raise EConfig("Sandbox.stop before boot")
        await self._session.stop()

    async def terminate(self) -> None:
        await self.stop()
