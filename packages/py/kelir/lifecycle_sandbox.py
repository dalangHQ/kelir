# see: cans/03-sdk-architecture.md#Sandbox-high-level-facade
from __future__ import annotations

import importlib
import tomllib
from pathlib import Path
from typing import TYPE_CHECKING, Any

import tomli_w
from pydantic import ValidationError

from kelir.profile_config import ProfileConfig
from kelir.sdk_errors import EConfig as EConfig

if TYPE_CHECKING:
    from kelir import CommitStats, KelirBuilder, Session, SpawnResult


def _builder_kwargs(model: ProfileConfig) -> dict[str, Any]:
    session = model.session
    if session is None:
        return {}
    kwargs: dict[str, Any] = {}
    if session.compute is not None:
        kwargs.update(
            vcpus=session.compute.vcpus,
            memory_mb=session.compute.memory_mb,
            page_size_kb=session.compute.page_size_kb,
        )
    if session.clock is not None:
        kwargs.update(
            tsc_locked=session.clock.tsc_locked,
            pinned_clocksource=session.clock.pinned_clocksource,
            deterministic_clock=session.clock.deterministic_clock,
            preemption_grace_ms=session.clock.preemption_grace_ms,
        )
    if session.watchdog is not None:
        kwargs.update(enable_watchdog=session.watchdog.enabled)
    if session.artifacts is not None:
        kwargs.update(
            kernel=session.artifacts.kernel_path,
            rootfs_erofs=session.artifacts.rootfs_erofs_path,
        )
    return {key: value for key, value in kwargs.items() if value is not None}


def _profile_text(model: ProfileConfig, path: str) -> str:
    data = model.model_dump(exclude_none=True)
    if path.endswith((".yaml", ".yml")):
        yaml = importlib.import_module("yaml")
        return str(yaml.safe_dump(data, sort_keys=False))
    return tomli_w.dumps(data)


class Sandbox:
    def __init__(
        self,
        builder: KelirBuilder,
        profile: ProfileConfig,
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
        return cls.from_profile(loaded, path)

    @classmethod
    def from_profile(cls, profile: dict[str, Any], source_path: str) -> Sandbox:
        from kelir import KelirBuilder

        try:
            model = ProfileConfig.model_validate(profile)
        except ValidationError as exc:
            raise EConfig(
                f"Sandbox.from_profile: invalid profile schema: {exc}"
            ) from exc
        return cls(KelirBuilder(**_builder_kwargs(model)), model, source_path)

    @classmethod
    async def create(
        cls,
        *,
        vcpus: int | None = None,
        memory_mb: int | None = None,
        page_size_kb: int | None = None,
        deterministic_clock: bool | None = None,
        pinned_clocksource: str | None = None,
        preemption_grace_ms: int | None = None,
        kernel: str | None = None,
        rootfs_erofs: str | None = None,
        tsc_locked: bool | None = None,
        enable_watchdog: bool | None = None,
        detached: bool = False,
    ) -> Sandbox:
        from kelir import KelirBuilder

        builder = KelirBuilder(
            vcpus=vcpus,
            memory_mb=memory_mb,
            page_size_kb=page_size_kb,
            deterministic_clock=deterministic_clock,
            pinned_clocksource=pinned_clocksource,
            preemption_grace_ms=preemption_grace_ms,
            kernel=kernel,
            rootfs_erofs=rootfs_erofs,
            tsc_locked=tsc_locked,
            enable_watchdog=enable_watchdog,
        ).detached(detached)
        return cls(builder, ProfileConfig(), "")

    @property
    def profile(self) -> dict[str, Any]:
        return self._profile.model_dump(exclude_none=True)

    @property
    def source_path(self) -> str:
        return self._source_path

    @property
    def session(self) -> Session | None:
        return self._session

    def session_id(self) -> str | None:
        session = self._profile.session
        return session.id if session is not None else None

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
        # TODO (L3): full-fidelity round-trip of arbitrary profile sections once plugin namespace schemas register.
        Path(path).write_text(_profile_text(self._profile, path), encoding="utf-8")

    async def stop(self) -> None:
        if self._session is None:
            raise EConfig("Sandbox.stop before boot")
        await self._session.stop()

    async def terminate(self) -> None:
        await self.stop()
