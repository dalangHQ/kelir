# see: cans/10-attestation.md#Execution-receipts
# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

from kelir.generated.attest_telemetry import (
    CausalDelta,
    ExecutionReceipt,
    ExitKindDirtyShutdown,
    ExitKindHardKilled,
    ExitKindHypervisorFault,
    ExitKindKernelPanic,
    ExitKindNormal,
    ExitKindSignal,
    GuestClock,
    PhaseTimings,
    ReceiptExtension,
    SnapshotDescriptor,
    from_native_exit_kind,
)


def test_exit_kind_defaults_to_normal() -> None:
    exit_kind = ExitKindNormal()
    assert exit_kind.kind == "normal"
    assert exit_kind.model_dump() == {"kind": "normal"}


def test_exit_kind_signal_shape() -> None:
    exit_kind = ExitKindSignal(signal=9)
    assert exit_kind.model_dump() == {"kind": "signal", "signal": 9}


def test_exit_kind_string_variants_carry_distinct_tags() -> None:
    assert ExitKindKernelPanic(message="oops").model_dump() == {
        "kind": "kernel-panic",
        "message": "oops",
    }
    assert ExitKindHypervisorFault(message="hv").model_dump() == {
        "kind": "hypervisor-fault",
        "message": "hv",
    }
    assert ExitKindDirtyShutdown().model_dump() == {"kind": "dirty-shutdown"}
    assert ExitKindHardKilled().model_dump() == {"kind": "hard-killed"}


def test_from_native_exit_kind_maps_camel_case_records() -> None:
    assert from_native_exit_kind("normal") == ExitKindNormal()
    assert from_native_exit_kind("dirtyShutdown") == ExitKindDirtyShutdown()
    assert from_native_exit_kind("hardKilled") == ExitKindHardKilled()
    assert from_native_exit_kind({"signal": 9}) == ExitKindSignal(signal=9)
    assert from_native_exit_kind({"kernelPanic": "oops"}) == ExitKindKernelPanic(
        message="oops"
    )
    assert from_native_exit_kind({"hypervisorFault": "hv"}) == ExitKindHypervisorFault(
        message="hv"
    )


def test_execution_receipt_defaults() -> None:
    receipt = ExecutionReceipt()
    assert receipt.pid == 0
    assert receipt.binary_path == ""
    assert receipt.binary_blake3 == ""
    assert receipt.exit_kind == ExitKindNormal()
    assert receipt.correlation_token is None
    assert receipt.dirty_bytes == 0
    assert receipt.syscall_count is None
    assert receipt.exit_code == 0
    assert receipt.wall_clock_ns == 0
    assert receipt.phase_timings is None
    assert receipt.egress_bytes == 0
    assert receipt.egress_dropped_bytes == 0
    assert receipt.extensions == []


def test_execution_receipt_field_roundtrip() -> None:
    receipt = ExecutionReceipt(
        pid=4242,
        binary_path="/usr/bin/git",
        binary_blake3="af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262",
        exit_kind=ExitKindSignal(signal=9),
        correlation_token="corr-001",
        dirty_bytes=4096,
        syscall_count=17,
        exit_code=137,
        wall_clock_ns=1_000_000_000,
        egress_bytes=512,
        egress_dropped_bytes=64,
    )
    assert receipt.pid == 4242
    assert receipt.exit_code == 137
    assert isinstance(receipt.exit_kind, ExitKindSignal)
    assert receipt.exit_kind.signal == 9
    dumped = receipt.model_dump()
    assert dumped["binary_path"] == "/usr/bin/git"
    assert dumped["exit_kind"] == {"kind": "signal", "signal": 9}
    assert dumped["extensions"] == []
    assert dumped["phase_timings"] is None


def test_execution_receipt_extensions_are_per_instance() -> None:
    first = ExecutionReceipt()
    second = ExecutionReceipt()
    assert first.extensions == []
    assert first.extensions is not second.extensions
    extension = ReceiptExtension(namespace="kelir/policy", payload=b'{"verdict":"drop"}')
    receipt = ExecutionReceipt(extensions=[extension])
    assert receipt.extensions[0].namespace == "kelir/policy"
    assert receipt.extensions[0].payload == b'{"verdict":"drop"}'
    assert receipt.model_dump()["extensions"] == [
        {"namespace": "kelir/policy", "payload": b'{"verdict":"drop"}'}
    ]


def test_phase_timings_default_to_zero() -> None:
    timings = PhaseTimings()
    assert timings.trap_ingress_ns == 0
    assert timings.ffi_hop_ns == 0
    assert timings.policy_ns == 0
    assert timings.verdict_egress_ns == 0
    timings = PhaseTimings(trap_ingress_ns=100, ffi_hop_ns=200, policy_ns=300, verdict_egress_ns=400)
    assert timings.model_dump() == {
        "trap_ingress_ns": 100,
        "ffi_hop_ns": 200,
        "policy_ns": 300,
        "verdict_egress_ns": 400,
    }


def test_guest_clock_defaults_and_shape() -> None:
    clock = GuestClock()
    assert clock.absolute_wall_clock_ns == 0
    assert clock.monotonic_offset_ns == 0
    assert clock.timezone == ""
    assert clock.boot_params == ""
    clock = GuestClock(
        absolute_wall_clock_ns=1_700_000_000_000_000_000,
        monotonic_offset_ns=-42,
        timezone="UTC",
        boot_params="tsc=reliable",
    )
    assert clock.model_dump() == {
        "absolute_wall_clock_ns": 1_700_000_000_000_000_000,
        "monotonic_offset_ns": -42,
        "timezone": "UTC",
        "boot_params": "tsc=reliable",
    }


def test_snapshot_descriptor_defaults_and_nesting() -> None:
    descriptor = SnapshotDescriptor()
    assert descriptor.id == ""
    assert descriptor.dirty_bytes == 0
    assert descriptor.created_at_ns == 0
    assert descriptor.parent_digest is None
    assert descriptor.guest_clock is None
    descriptor = SnapshotDescriptor(
        id="snap-001",
        dirty_bytes=2048,
        created_at_ns=99,
        parent_digest="b3:abc",
        guest_clock=GuestClock(timezone="UTC"),
    )
    assert descriptor.model_dump()["guest_clock"] == {
        "absolute_wall_clock_ns": 0,
        "monotonic_offset_ns": 0,
        "timezone": "UTC",
        "boot_params": "",
    }


def test_causal_delta_defaults() -> None:
    delta = CausalDelta()
    assert delta.memory_diff_bytes == 0
    assert delta.modified_inodes == []
    assert delta.spawned_pids == []
    assert delta.egress_bytes == 0
    assert delta.storage_delta_root == ""
    assert delta.receipt_leaf_hash == ""


def test_causal_delta_field_roundtrip() -> None:
    delta = CausalDelta(
        memory_diff_bytes=65536,
        modified_inodes=["/tmp/a", "/tmp/b"],
        spawned_pids=[11, 22],
        egress_bytes=128,
        storage_delta_root="b3:deadbeef",
        receipt_leaf_hash="b3:cafe",
    )
    assert delta.model_dump() == {
        "memory_diff_bytes": 65536,
        "modified_inodes": ["/tmp/a", "/tmp/b"],
        "spawned_pids": [11, 22],
        "egress_bytes": 128,
        "storage_delta_root": "b3:deadbeef",
        "receipt_leaf_hash": "b3:cafe",
    }
    assert CausalDelta().modified_inodes is not CausalDelta().modified_inodes
