# see: cans/03-sdk-architecture.md#Sandbox-high-level-facade
from __future__ import annotations

import importlib
import json
import tomllib
from pathlib import Path
from typing import TYPE_CHECKING, Any, NamedTuple

from kelir.sdk_errors import EConfig as EConfig

if TYPE_CHECKING:
    from kelir import CommitStats, KelirBuilder, Session, SpawnResult


class ProfileField(NamedTuple):
    path: tuple[str, ...]
    kind: str
    kwarg: str | None


# see: cans/plugins/profile.md#Session-compute-schema-(profile)
# see: cans/plugins/profile.md#Invariants-and-policy-(profile)
PROFILE_FIELDS: tuple[ProfileField, ...] = (
    ProfileField(("session", "id"), "string", None),
    ProfileField(("session", "compute", "vcpus"), "integer", "vcpus"),
    ProfileField(("session", "compute", "memory_mb"), "integer", "memory_mb"),
    ProfileField(("session", "artifacts", "kernel_path"), "string", "kernel"),
    ProfileField(
        ("session", "artifacts", "rootfs_erofs_path"), "string", "rootfs_erofs"
    ),
    ProfileField(("session", "clock", "tsc_locked"), "boolean", "tsc_locked"),
    ProfileField(("session", "watchdog", "enabled"), "boolean", "enable_watchdog"),
)


def _dig(profile: dict[str, Any], path: tuple[str, ...]) -> Any:
    node: Any = profile
    for key in path:
        if not isinstance(node, dict):
            return None
        node = node.get(key)
    return node


def _matches(value: Any, kind: str) -> bool:
    if kind == "boolean":
        return isinstance(value, bool)
    if kind == "integer":
        return isinstance(value, int) and not isinstance(value, bool)
    return isinstance(value, str)


def _render_toml_value(kind: str, value: Any) -> str:
    if kind == "boolean":
        return "true" if value else "false"
    if kind == "integer":
        return str(value)
    return json.dumps(value)


def _render_toml(profile: dict[str, Any]) -> str:
    lines: list[str] = []
    section: str | None = None
    for field in PROFILE_FIELDS:
        value = _dig(profile, field.path)
        if not _matches(value, field.kind):
            continue
        head = ".".join(field.path[:-1])
        if head != section:
            if lines:
                lines.append("")
            lines.append(f"[{head}]")
            section = head
        lines.append(f"{field.path[-1]} = {_render_toml_value(field.kind, value)}")
    return "\n".join(lines) + ("\n" if lines else "")


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
            loaded: Any = tomllib.loads(text)
        elif path.endswith((".yaml", ".yml")):
            yaml = importlib.import_module("yaml")
            loaded = yaml.safe_load(text)
        else:
            raise EConfig(
                f"Sandbox.from_config: unsupported profile extension for `{path}` (expected .toml, .yaml, or .yml)"
            )
        profile = loaded if isinstance(loaded, dict) else {}
        return cls.from_profile(profile, path)

    @classmethod
    def from_profile(cls, profile: dict[str, Any], source_path: str) -> Sandbox:
        from kelir import KelirBuilder

        kwargs: dict[str, Any] = {}
        for field in PROFILE_FIELDS:
            value = _dig(profile, field.path)
            if field.kwarg is not None and _matches(value, field.kind):
                kwargs[field.kwarg] = value
        return cls(KelirBuilder(**kwargs), profile, source_path)

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
        value = _dig(self._profile, ("session", "id"))
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
        # TODO (L3): full-fidelity round-trip of arbitrary profile sections via a declared TOML writer.
        Path(path).write_text(_render_toml(self._profile), encoding="utf-8")

    async def stop(self) -> None:
        if self._session is None:
            raise EConfig("Sandbox.stop before boot")
        await self._session.stop()

    async def terminate(self) -> None:
        await self.stop()
