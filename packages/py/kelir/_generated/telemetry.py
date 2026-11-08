# see: cans/03-sdk-architecture.md#Python-SDK
# see: cans/15-operations.md#Multi-language-parity-gate
# see: cans/10-attestation.md#Execution-receipts
from __future__ import annotations

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


class ExitKind(BaseModel):
    kind: str = "normal"
    signal: int | None = None
    message: str | None = None


class ExecutionReceipt(BaseModel):
    pid: int = 0
    binary_path: str = ""
    binary_blake3: str = ""
    exit_kind: ExitKind = Field(default_factory=ExitKind)
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


