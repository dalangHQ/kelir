// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/15-operations.md#Multi-language-parity-gate
// see: cans/10-attestation.md#Execution-receipts

export interface GuestClock {
  readonly absoluteWallClockNs: number;
  readonly monotonicOffsetNs: number;
  readonly timezone: string;
  readonly bootParams: string;
}

export interface SnapshotDescriptor {
  readonly id: string;
  readonly dirtyBytes: number;
  readonly createdAtNs: number;
  readonly parentDigest?: string | null;
  readonly guestClock?: GuestClock | null;
}

export interface ReceiptExtension {
  readonly namespace: string;
  readonly payload: Uint8Array;
}

export interface PhaseTimings {
  readonly trapIngressNs: number;
  readonly ffiHopNs: number;
  readonly policyNs: number;
  readonly verdictEgressNs: number;
}

export type ExitKind =
  | { readonly kind: "normal" }
  | { readonly kind: "signal"; readonly signal: number }
  | { readonly kind: "kernel-panic"; readonly message: string }
  | { readonly kind: "hypervisor-fault"; readonly message: string }
  | { readonly kind: "dirty-shutdown" }
  | { readonly kind: "hard-killed" };

export interface ExecutionReceipt {
  readonly pid: number;
  readonly binaryPath: string;
  readonly binaryBlake3: string;
  readonly exitKind: ExitKind;
  readonly correlationToken?: string | null;
  readonly dirtyBytes: number;
  readonly syscallCount?: number | null;
  readonly exitCode: number;
  readonly wallClockNs: number;
  readonly phaseTimings?: PhaseTimings | null;
  readonly egressBytes: number;
  readonly egressDroppedBytes: number;
  readonly extensions: readonly ReceiptExtension[];
}

export interface CausalDelta {
  readonly memoryDiffBytes: number;
  readonly modifiedInodes: readonly string[];
  readonly spawnedPids: readonly number[];
  readonly egressBytes: number;
  readonly storageDeltaRoot: string;
  readonly receiptLeafHash: string;
}

export interface NativeGuestClockRecord {
  readonly absolute_wall_clock_ns: number;
  readonly monotonic_offset_ns: number;
  readonly timezone: string;
  readonly boot_params: string;
}

export interface NativeSnapshotRecord {
  readonly id: string;
  readonly dirty_bytes: number;
  readonly created_at_ns: number;
  readonly parent_digest?: string | null;
  readonly guest_clock?: NativeGuestClockRecord | null;
}

export interface NativePhaseTimingsRecord {
  readonly trapIngressNs: number;
  readonly ffiHopNs: number;
  readonly policyNs: number;
  readonly verdictEgressNs: number;
}

export type NativeExitKindRecord =
  | "normal"
  | { readonly signal: number }
  | { readonly kernelPanic: string }
  | { readonly hypervisorFault: string }
  | "dirtyShutdown"
  | "hardKilled";

export function fromNativeExitKind(raw: NativeExitKindRecord): ExitKind {
  if (raw === "normal") return { kind: "normal" };
  if (raw === "dirtyShutdown") return { kind: "dirty-shutdown" };
  if (raw === "hardKilled") return { kind: "hard-killed" };
  if (typeof raw === "object" && "signal" in raw) {
    return { kind: "signal", signal: raw.signal };
  }
  if (typeof raw === "object" && "kernelPanic" in raw) {
    return { kind: "kernel-panic", message: raw.kernelPanic };
  }
  return { kind: "hypervisor-fault", message: raw.hypervisorFault };
}

export interface NativeExecutionReceiptRecord {
  readonly pid: number;
  readonly binaryPath: string;
  readonly binaryBlake3: string;
  readonly exitKind: NativeExitKindRecord;
  readonly correlationToken?: string | null;
  readonly dirtyBytes: number;
  readonly syscallCount?: number | null;
  readonly exitCode: number;
  readonly wallClockNs: number;
  readonly phaseTimings?: NativePhaseTimingsRecord | null;
  readonly egressBytes: number;
  readonly egressDroppedBytes: number;
  readonly extensions: readonly ReceiptExtension[];
}

export interface NativeCausalDeltaRecord {
  readonly memoryDiffBytes: number;
  readonly modifiedInodes: readonly string[];
  readonly spawnedPids: readonly number[];
  readonly egressBytes: number;
  readonly storageDeltaRoot: string;
  readonly receiptLeafHash: string;
}

export function fromNativeGuestClock(raw: NativeGuestClockRecord): GuestClock {
  return {
    absoluteWallClockNs: raw.absolute_wall_clock_ns,
    monotonicOffsetNs: raw.monotonic_offset_ns,
    timezone: raw.timezone,
    bootParams: raw.boot_params,
  };
}

export function fromNativeSnapshotDescriptor(raw: NativeSnapshotRecord): SnapshotDescriptor {
  return {
    id: raw.id,
    dirtyBytes: raw.dirty_bytes,
    createdAtNs: raw.created_at_ns,
    parentDigest: raw.parent_digest ?? null,
    guestClock: raw.guest_clock ? fromNativeGuestClock(raw.guest_clock) : null,
  };
}

export function fromNativePhaseTimings(raw: NativePhaseTimingsRecord): PhaseTimings {
  return {
    trapIngressNs: raw.trapIngressNs,
    ffiHopNs: raw.ffiHopNs,
    policyNs: raw.policyNs,
    verdictEgressNs: raw.verdictEgressNs,
  };
}

export function fromNativeExecutionReceipt(raw: NativeExecutionReceiptRecord): ExecutionReceipt {
  return {
    pid: raw.pid,
    binaryPath: raw.binaryPath,
    binaryBlake3: raw.binaryBlake3,
    exitKind: fromNativeExitKind(raw.exitKind),
    correlationToken: raw.correlationToken ?? null,
    dirtyBytes: raw.dirtyBytes,
    syscallCount: raw.syscallCount ?? null,
    exitCode: raw.exitCode,
    wallClockNs: raw.wallClockNs,
    phaseTimings: raw.phaseTimings ? fromNativePhaseTimings(raw.phaseTimings) : null,
    egressBytes: raw.egressBytes,
    egressDroppedBytes: raw.egressDroppedBytes,
    extensions: raw.extensions,
  };
}

export function fromNativeCausalDelta(raw: NativeCausalDeltaRecord): CausalDelta {
  return {
    memoryDiffBytes: raw.memoryDiffBytes,
    modifiedInodes: raw.modifiedInodes,
    spawnedPids: raw.spawnedPids,
    egressBytes: raw.egressBytes,
    storageDeltaRoot: raw.storageDeltaRoot,
    receiptLeafHash: raw.receiptLeafHash,
  };
}

