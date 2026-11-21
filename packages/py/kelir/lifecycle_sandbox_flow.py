# see: cans/03-sdk-architecture.md#Sandbox-high-level-facade
from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING, Any

from pydantic import ValidationError

from kelir.profile_config import ProfileConfig
from kelir.sdk_errors import EConfig as EConfig

if TYPE_CHECKING:
    from kelir import (
        CommitStats,
        KelirBuilder,
        Session,
        SnapshotDescriptor,
        SpawnResult,
        SuspendReason,
    )


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


# see: cans/plugins/profile.md#Declarative-profile-execution
def _parse_profile(origin: str, path: str) -> dict[str, Any]:
    return _parse_profile_text(origin, path, _read_profile_text(origin, path))


# see: cans/plugins/profile.md#Session-compute-schema-(profile)
def _profile_model(origin: str, profile: dict[str, Any]) -> ProfileConfig:
    try:
        return ProfileConfig.model_validate(profile)
    except ValidationError as exc:
        raise EConfig(f"{origin}: invalid profile schema: {exc}") from exc


from kelir.lifecycle_sandbox_run import (  # see: cans/plugins/profile.md#Declarative-profile-execution
    _parse_profile_text,
    _profile_text,
    _read_profile_text,
    _write_profile_text,
)


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
        return cls.from_profile(_parse_profile("Sandbox.from_config", path), path)

    @classmethod
    def from_profile(cls, profile: dict[str, Any], source_path: str) -> Sandbox:
        from kelir import KelirBuilder

        model = _profile_model("Sandbox.from_profile", profile)
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

    async def rollback(self, snapshot: SnapshotDescriptor | None = None) -> None:
        if self._session is None:
            raise EConfig("Sandbox.rollback before boot")
        await self._session.rollback(snapshot)

    # see: cans/03-sdk-architecture.md#Sandbox-high-level-facade
    async def suspend(self, reason: SuspendReason) -> None:
        if self._session is None:
            raise EConfig("Sandbox.suspend before boot")
        await self._session.suspend(reason)

    async def resume(self) -> None:
        if self._session is None:
            raise EConfig("Sandbox.resume before boot")
        await self._session.resume()

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
        # TODO (L2) [profile_ingestion]: overlay programmatic plane descriptors over the ingested profile before export.; slop: post-ingestion descriptor overrides silently absent from the export
        _write_profile_text(path, _profile_text(self._profile, path))

    async def stop(self) -> None:
        if self._session is None:
            raise EConfig("Sandbox.stop before boot")
        await self._session.stop()

    # see: cans/08-lifecycle.md#Pre-teardown-flush
    async def terminate(self) -> None:
        if self._session is None:
            raise EConfig("Sandbox.terminate before boot")
        await self._session.terminate()
