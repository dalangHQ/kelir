# see: cans/plugins/profile.md#Session-compute-schema-(profile)
# see: cans/plugins/profile.md#Invariants-and-policy-(profile)
# see: cans/artifacts/examples/fixtures/kelir.yaml#session
from __future__ import annotations

from typing import Annotated, Any

from pydantic import BaseModel, BeforeValidator, ConfigDict, Field


def _profile_int_guard(value: Any) -> Any:
    if isinstance(value, (bool, str)):
        raise ValueError("profile integers must not be boolean or string values")
    return value


ProfileInt = Annotated[int, BeforeValidator(_profile_int_guard)]
ProfileBool = Annotated[bool, Field(strict=True)]

Region = dict[str, bool | int | str]


class ComputeProfile(BaseModel):
    vcpus: ProfileInt | None = None
    max_vcpus: ProfileInt | None = None
    memory_mb: ProfileInt | None = None
    page_size_kb: ProfileInt | None = None


class BalloonProfile(BaseModel):
    enabled: ProfileBool | None = None
    initial_mb: ProfileInt | None = None
    pressure_reclaim: ProfileBool | None = None
    reclaim_strategy: str | None = None


class ClockProfile(BaseModel):
    tsc_locked: ProfileBool | None = None
    pinned_clocksource: str | None = None
    deterministic_clock: ProfileBool | None = None
    preemption_grace_ms: ProfileInt | None = None


class WatchdogProfile(BaseModel):
    enabled: ProfileBool | None = None
    timeout_seconds: ProfileInt | None = None
    action: str | None = None
    heartbeat_cadence_seconds: ProfileInt | None = None


class VirtioRngProfile(BaseModel):
    enabled: ProfileBool | None = None
    source: str | None = None


class VirtioShmProfile(BaseModel):
    enabled: ProfileBool | None = None
    regions: list[Region] | None = None


class DevicesProfile(BaseModel):
    virtio_rng: VirtioRngProfile | None = None
    virtio_shm: VirtioShmProfile | None = None


class ArtifactsProfile(BaseModel):
    kernel_path: str | None = None
    rootfs_erofs_path: str | None = None
    kernel_boot_params: list[str] | None = None


class ShimProfile(BaseModel):
    max_exec_slots: ProfileInt | None = None
    hide_pid: ProfileInt | None = None
    anti_toctou_cgroups: ProfileBool | None = None


class DriverProfile(BaseModel):
    fast_boot: str | None = None


class StorageProfile(BaseModel):
    dax_window: str | None = None


class SessionProfile(BaseModel):
    id: str | None = None
    topology: str | None = None
    compute: ComputeProfile | None = None
    balloon: BalloonProfile | None = None
    clock: ClockProfile | None = None
    watchdog: WatchdogProfile | None = None
    devices: DevicesProfile | None = None
    artifacts: ArtifactsProfile | None = None
    shim: ShimProfile | None = None
    driver: DriverProfile | None = None
    storage: StorageProfile | None = None


# TODO (L3) [bindings_py]: route non-session namespaces to their ConfigurablePlugin schema owners per cans/plugins/profile.md#Inversion-of-Control-configuration-router.; slop: non-session namespace silently dropped from config
class ProfileConfig(BaseModel):
    model_config = ConfigDict(extra="forbid")

    version: ProfileInt | None = None
    invariants: dict[str, Any] | None = None
    driver: dict[str, Any] | None = None
    session: SessionProfile | None = None
    storage: dict[str, Any] | None = None
    wire: dict[str, Any] | None = None
    terminal: dict[str, Any] | None = None
    checkpoint: dict[str, Any] | None = None
    host: dict[str, Any] | None = None
    plugins: dict[str, Any] | None = None
    attestation: dict[str, Any] | None = None
