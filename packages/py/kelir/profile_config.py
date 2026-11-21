# see: cans/plugins/profile.md#Session-compute-schema-(profile)
# see: cans/plugins/profile.md#Invariants-and-policy-(profile)
# see: cans/artifacts/examples/fixtures/kelir.yaml#session
from __future__ import annotations

from typing import Annotated, Any, Self

from pydantic import BaseModel, BeforeValidator, ConfigDict, Field, model_validator


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

    # see: cans/plugins/profile.md#Session-balloon-(profile)
    def to_vm_balloon_config(self) -> VmBalloonConfig:
        return VmBalloonConfig(
            enabled=self.enabled if self.enabled is not None else False,
            initial_mb=self.initial_mb if self.initial_mb is not None else 0,
            pressure_reclaim=self.pressure_reclaim
            if self.pressure_reclaim is not None
            else False,
            reclaim_strategy=self.reclaim_strategy
            if self.reclaim_strategy is not None
            else "",
        )


# see: cans/artifacts/examples/fixtures/kelir.yaml#session.balloon
class VmBalloonConfig(BaseModel):
    enabled: bool = False
    initial_mb: int = 0
    pressure_reclaim: bool = False
    reclaim_strategy: str = ""


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


class IsolationEnvelopeProfile(BaseModel):
    enforce_openat2_beneath: ProfileBool | None = None
    cgroup_parent: str | None = None


class BudgetProfile(BaseModel):
    exec_seconds: ProfileInt | None = None


class ShimProfile(BaseModel):
    max_exec_slots: ProfileInt | None = None
    hide_pid: ProfileInt | None = None
    anti_toctou_cgroups: ProfileBool | None = None
    granular_task_freeze: ProfileBool | None = None
    task_freeze_timeout_ms: ProfileInt | None = None
    rejection_mode: str | None = None
    mask_proc_environ: ProfileBool | None = None
    # Leaf cgroup v2 isolates pids.max/memory.max per ExecSlot
    per_exec_cgroup: ProfileBool | None = None
    # Enforces native AUDIT_ARCH; rejects x32/compat syscalls
    deny_abi_compat: ProfileBool | None = None
    blocked_syscalls: list[str] | None = None
    blocked_socket_families: list[str] | None = None


# see: cans/artifacts/examples/fixtures/kelir.yaml#session.steering
class SteeringProfile(BaseModel):
    # Emits structured agent guidance on Drop
    steer_on_deny: ProfileBool | None = None
    # Announces Patch redirects on EXEC/L7/SQL
    steer_on_redirect: ProfileBool | None = None
    # Flushes steering lines to EXEC sub-lane 0x02
    inject_exec_stderr: ProfileBool | None = None


# see: cans/artifacts/examples/fixtures/kelir.yaml#wire.dns
class DnsProfile(BaseModel):
    # Correlates allowed domain answers to TTL-bounded L4 IPs
    dns_to_ip_pinning: ProfileBool | None = None


# see: cans/artifacts/examples/fixtures/kelir.yaml#storage.lock_broker
class LockBrokerProfile(BaseModel):
    enabled: ProfileBool | None = None
    non_blocking: ProfileBool | None = None
    # Bounded SETLKW waiter pool ceiling before EDEADLK
    blocking_wait_timeout_ms: ProfileInt | None = None


class DriverProfile(BaseModel):
    fast_boot: str | None = None


class StorageProfile(BaseModel):
    dax_window: str | None = None
    lock_broker: LockBrokerProfile | None = None


class SessionProfile(BaseModel):
    id: str | None = None
    tenant_id: str | None = None
    topology: str | None = None
    isolation_envelope: IsolationEnvelopeProfile | None = None
    budget: BudgetProfile | None = None
    compute: ComputeProfile | None = None
    balloon: BalloonProfile | None = None
    clock: ClockProfile | None = None
    watchdog: WatchdogProfile | None = None
    devices: DevicesProfile | None = None
    artifacts: ArtifactsProfile | None = None
    shim: ShimProfile | None = None
    steering: SteeringProfile | None = None
    driver: DriverProfile | None = None
    storage: StorageProfile | None = None

    # see: cans/plugins/profile.md#Monotonic-limits
    @model_validator(mode="after")
    def _balloon_within_memory_ceiling(self) -> Self:
        if (
            self.compute is not None
            and self.balloon is not None
            and self.compute.memory_mb is not None
            and self.balloon.initial_mb is not None
            and self.balloon.initial_mb > self.compute.memory_mb
        ):
            raise ValueError(
                "session.balloon.initial_mb "
                f"{self.balloon.initial_mb} exceeds session.compute.memory_mb "
                f"{self.compute.memory_mb}"
            )
        return self


# L3-GAP: [bindings_py]: route non-session namespaces to their ConfigurablePlugin schema owners per cans/plugins/profile.md#Inversion-of-Control-configuration-router.; slop: non-session namespace silently dropped from config
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
