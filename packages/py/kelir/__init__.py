# see: cans/03-sdk-architecture.md#Python-SDK
# see: cans/02-plugin-model.md#Foreign-plugin-proxy
from __future__ import annotations

import importlib
import re
from dataclasses import dataclass, field
from datetime import timedelta
from enum import Enum
from typing import Any, Callable, Generic, TypeVar, Union

CtxT = TypeVar("CtxT")
ParamsT = TypeVar("ParamsT")

from kelir.generated.wire_protocol import Channel, ExecStream, MsgType, StreamKind
from kelir.generated.attest_telemetry import (
    CausalDelta as CausalDelta,
    ExecutionReceipt as ExecutionReceipt,
    ExitKind as ExitKind,
    ExitKindNormal as ExitKindNormal,
    GuestClock as GuestClock,
    PhaseTimings as PhaseTimings,
    ReceiptExtension as ReceiptExtension,
    SnapshotDescriptor as SnapshotDescriptor,
)
from kelir.sdk_errors_flow import EConfig as EConfig
from kelir.lifecycle_sandbox_flow import Sandbox as Sandbox
from kelir.plugin_fusion_flow import TrapFilterSnapshot

from kelir._kelir_flow import (
    Approval as Approval,
    Condition as Condition,
    _bridge_safe_config as _bridge_safe_config,
    _load_native_bridge as _load_native_bridge,
    _require_native as _require_native,
    _snapshot_from_dict as _snapshot_from_dict,
    _snapshot_to_dict as _snapshot_to_dict,
    _suspend_until as _suspend_until,
    _verify_native_bridge_abi as _verify_native_bridge_abi,
    CheckpointHandle as CheckpointHandle,
    ClockHandle as ClockHandle,
    Deadline as Deadline,
    Errno as Errno,
    ExecveMutation as ExecveMutation,
    FsHandle as FsHandle,
    HandlerContext as HandlerContext,
    HostHandle as HostHandle,
    NativeAddonAbiMismatchError as NativeAddonAbiMismatchError,
    NativeAddonMissingError as NativeAddonMissingError,
    Handshake as Handshake,
    PatchPayload as PatchPayload,
    PostExecAction as PostExecAction,
    StepHandle as StepHandle,
    StepVerdict as StepVerdict,
    StorageHandle as StorageHandle,
    TrapEvent as TrapEvent,
    TerminalHandle as TerminalHandle,
    TrapFilter as TrapFilter,
    Verdict as Verdict,
    VmHandle as VmHandle,
    VsockChannel as VsockChannel,
    VsockHandle as VsockHandle,
    WireHandle as WireHandle,
    relay_socket_path as relay_socket_path,
)



# see: cans/03-sdk-architecture.md#Python-SDK
from pydantic import BaseModel as BaseModel, Field as Field


# see: cans/03-sdk-architecture.md#Python-SDK
KELIR_ADDON_ABI_VERSION = 1

# see: cans/03-sdk-architecture.md#Python-SDK
NATIVE_BRIDGE_MODULES: tuple[str, ...] = ("kelir._kelir", "_kelir")











# see: cans/11-protocol.md#Wire-messages
SuspendUntil = Approval | Deadline | Handshake | Condition



# see: cans/08-lifecycle.md#Suspend-and-resume
ExternalSuspendKind = Union[str, int]


# see: cans/artifacts/code/vmm-driver-trait.rs#SuspendReason
@dataclass
class External:
    kind: ExternalSuspendKind


# see: cans/artifacts/code/vmm-driver-trait.rs#SuspendReason
@dataclass
class ResourcePressure:
    pass


# see: cans/artifacts/code/vmm-driver-trait.rs#SuspendReason
SuspendReason = External | ResourcePressure


# see: cans/08-lifecycle.md#Watchdog
class WatchdogAction(str, Enum):
    REBOOT = "reboot"
    TERMINATE = "terminate"


# see: cans/08-lifecycle.md#State-machine
class LifecycleEvent(str, Enum):
    VmInit = "vm-init"
    VmReady = "vm-ready"
    VmPreempt = "vm-preempt"
    VmShutdown = "vm-shutdown"



# see: cans/05-storage-plane.md#Interception-pipeline
# see: cans/02-plugin-model.md#Unified-verdict



# see: cans/02-plugin-model.md#Unified-verdict
FilterAction = Verdict





# see: cans/03-sdk-architecture.md#Idiomatic-Surfaces-Law
@dataclass
class SpawnResult:
    exit_code: int = 0
    stdout: bytes = b""
    stderr: bytes = b""
    # see: cans/10-attestation.md#Execution-receipts
    receipt: ExecutionReceipt | None = None
    exit_kind: ExitKind = field(default_factory=ExitKindNormal)


@dataclass
class CommitStats:
    files_changed: int = 0
    tombstones_resolved: int = 0


# see: cans/03-sdk-architecture.md#Service-lookup-across-languages
from kelir.service_caps_flow import (
    Caps as Caps,
    ContainerCap as ContainerCap,
    ContainerHandle as ContainerHandle,
    DbCap as DbCap,
    DbProxyHandle as DbProxyHandle,
    LedgerCap as LedgerCap,
    LedgerFinalizeResult as LedgerFinalizeResult,
    LedgerHandle as LedgerHandle,
    MeshHandle as MeshHandle,
    RepoPolicyCap as RepoPolicyCap,
    Savepoint as Savepoint,
    SqlProxyHandle as SqlProxyHandle,
    TransactionCap as TransactionCap,
    TransactionHandle as TransactionHandle,
    TransactionScope as TransactionScope,
    Tx as Tx,
)
















from kelir.lifecycle_session_flow import Session as Session
from kelir.kelir_builder_flow import (
    KelirBuilder as KelirBuilder,
    VsockServiceEntry as VsockServiceEntry,
    builder as builder,
    kelir as kelir,
)
from kelir.plugin_model_flow import DirEntry as DirEntry, DnsVerdict as DnsVerdict


__all__ = [
    "Approval",
    "BaseModel",
    "Caps",
    "CausalDelta",
    "Channel",
    "CheckpointHandle",
    "ContainerCap",
    "ContainerHandle",
    "CtxT",
    "DbCap",
    "DbProxyHandle",
    "Deadline",
    "DirEntry",
    "DnsVerdict",
    "Errno",
    "ExecStream",
    "ExecveMutation",
    "ExecutionReceipt",
    "ExitKind",
    "External",
    "ExternalSuspendKind",
    "Field",
    "FilterAction",
    "GuestClock",
    "Handshake",
    "HandlerContext",
    "HostHandle",
    "KelirBuilder",
    "LedgerCap",
    "LedgerFinalizeResult",
    "LedgerHandle",
    "LifecycleEvent",
    "MeshHandle",
    "MsgType",
    "ParamsT",
    "PatchPayload",
    "PhaseTimings",
    "PostExecAction",
    "ReceiptExtension",
    "RepoPolicyCap",
    "ResourcePressure",
    "CommitStats",
    "Sandbox",
    "Savepoint",
    "Session",
    "SnapshotDescriptor",
    "SpawnResult",
    "SqlProxyHandle",
    "StepVerdict",
    "StreamKind",
    "SuspendReason",
    "Condition",
    "SuspendUntil",
    "TrapFilter",
    "TransactionCap",
    "TransactionHandle",
    "TransactionScope",
    "TrapEvent",
    "Tx",
    "Verdict",
    "WatchdogAction",
    "builder",
    "kelir",
    "relay_socket_path",
]
