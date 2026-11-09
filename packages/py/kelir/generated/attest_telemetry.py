# see: cans/03-sdk-architecture.md#Python-SDK
# see: cans/15-operations.md#Multi-language-parity-gate
# see: cans/10-attestation.md#Execution-receipts
from __future__ import annotations

from typing import Literal, cast

from pydantic import BaseModel, Field

class GuestClock(BaseModel):
    absolute_wall_clock_ns: int = 0
    monotonic_offset_ns: int = 0
    timezone: str = ""
    boot_params: str = ""


class SnapshotDescriptor(BaseModel):
    id: str = ""
    dirty_bytes: int = 0
    created_at_ns: int = 0
    parent_digest: str | None = None
    guest_clock: GuestClock | None = None


class ReceiptExtension(BaseModel):
    namespace: str = ""
    payload: bytes = b""


class PhaseTimings(BaseModel):
    trap_ingress_ns: int = 0
    ffi_hop_ns: int = 0
    policy_ns: int = 0
    verdict_egress_ns: int = 0


class ExitKindNormal(BaseModel):
    kind: Literal["normal"] = "normal"


class ExitKindSignal(BaseModel):
    kind: Literal["signal"] = "signal"
    signal: int = 0


class ExitKindKernelPanic(BaseModel):
    kind: Literal["kernel-panic"] = "kernel-panic"
    message: str = ""


class ExitKindHypervisorFault(BaseModel):
    kind: Literal["hypervisor-fault"] = "hypervisor-fault"
    message: str = ""


class ExitKindDirtyShutdown(BaseModel):
    kind: Literal["dirty-shutdown"] = "dirty-shutdown"


class ExitKindHardKilled(BaseModel):
    kind: Literal["hard-killed"] = "hard-killed"


ExitKind = (
    ExitKindNormal
    | ExitKindSignal
    | ExitKindKernelPanic
    | ExitKindHypervisorFault
    | ExitKindDirtyShutdown
    | ExitKindHardKilled
)

NativeExitKindRecord = str | dict[str, int | str]


def from_native_exit_kind(raw: NativeExitKindRecord) -> ExitKind:
    if isinstance(raw, str):
        if raw == "normal":
            return ExitKindNormal()
        if raw == "dirtyShutdown":
            return ExitKindDirtyShutdown()
        if raw == "hardKilled":
            return ExitKindHardKilled()
        raise ValueError(f"unknown native exit kind tag: {raw}")
    if "signal" in raw:
        return ExitKindSignal(signal=cast(int, raw["signal"]))
    if "kernelPanic" in raw:
        return ExitKindKernelPanic(message=cast(str, raw["kernelPanic"]))
    return ExitKindHypervisorFault(message=cast(str, raw["hypervisorFault"]))


# TODO (L3): no FFI producer populates ExecutionReceipt.exit_kind yet
class ExecutionReceipt(BaseModel):
    pid: int = 0
    binary_path: str = ""
    binary_blake3: str = ""
    exit_kind: ExitKind = Field(default_factory=ExitKindNormal)
    correlation_token: str | None = None
    dirty_bytes: int = 0
    syscall_count: int | None = None
    exit_code: int = 0
    wall_clock_ns: int = 0
    phase_timings: PhaseTimings | None = None
    egress_bytes: int = 0
    egress_dropped_bytes: int = 0
    extensions: list[ReceiptExtension] = Field(default_factory=list)


class CausalDelta(BaseModel):
    memory_diff_bytes: int = 0
    modified_inodes: list[str] = Field(default_factory=list)
    spawned_pids: list[int] = Field(default_factory=list)
    egress_bytes: int = 0
    storage_delta_root: str = ""
    receipt_leaf_hash: str = ""


