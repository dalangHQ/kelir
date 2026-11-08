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

export interface CausalDelta {
  readonly memoryDiffBytes: number;
  readonly modifiedInodes: readonly string[];
  readonly spawnedPids: readonly number[];
  readonly egressBytes: number;
  readonly storageDeltaRoot: string;
  readonly receiptLeafHash: string;
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

export interface SpawnOptions {
  readonly cwd?: string;
  readonly params?: Record<string, unknown>;
}

export interface SpawnResult {
  readonly exitCode: number;
  readonly stdout?: string;
  readonly stderr?: string;
}

export interface TrapInfo {
  readonly pid: number;
  readonly ppid: number;
  readonly binaryPath: string;
  readonly binaryBlake3: string;
  readonly argv: string[];
  readonly cwd: string;
  readonly syscallNr?: number;
  readonly protFlags?: number;
  readonly memfdFlags?: number;
}

export interface FsInodeAttr {
  readonly mode: number;
  readonly uid: number;
  readonly gid: number;
  readonly size: number;
  readonly content: string;
}

export interface ExecveMutation {
  readonly binary: string;
  readonly argv: string[];
  readonly env?: Record<string, string>;
}

export interface CustomActionPayload {
  readonly name: string;
  readonly cbor: unknown;
}

export type PatchInput = ExecveMutation | CustomActionPayload | FsInodeAttr;

export interface HeartbeatEvent {
  readonly uptimeMs: number;
}

export interface ProcessExitEvent {
  readonly pid?: number;
  readonly exitCode: number;
}

export type WatchdogAction = "reboot" | "terminate";

export type WatchdogVerdict = "mitigate" | "suppress" | "execute-default";

export interface WatchdogEvent {
  readonly timeoutMs: number;
  readonly action: WatchdogAction;
  readonly firedAtNs: number;
  readonly lastHeartbeatMs: number;
  readonly lastActivePid?: number | null;
}

export interface TrapDenyEvent {
  readonly pid: number;
  readonly ppid: number;
  readonly binaryPath: string;
  readonly reason: string;
}

// see: cans/15-operations.md#Multi-language-parity-gate
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

export interface NativeCausalDeltaRecord {
  readonly memory_diff_bytes: number;
  readonly modified_inodes: readonly string[];
  readonly spawned_pids: readonly number[];
  readonly egress_bytes: number;
  readonly storage_delta_root: string;
  readonly receipt_leaf_hash: string;
}

export interface NativePhaseTimingsRecord {
  readonly trap_ingress_ns: number;
  readonly ffi_hop_ns: number;
  readonly policy_ns: number;
  readonly verdict_egress_ns: number;
}

export type NativeExitKindRecord =
  | "Normal"
  | "DirtyShutdown"
  | "HardKilled"
  | { readonly Signal: number }
  | { readonly KernelPanic: string }
  | { readonly HypervisorFault: string };

export interface NativeExecutionReceiptRecord {
  readonly pid: number;
  readonly binary_path: string;
  readonly binary_blake3: string;
  readonly exit_kind: NativeExitKindRecord;
  readonly correlation_token?: string | null;
  readonly dirty_bytes: number;
  readonly syscall_count?: number | null;
  readonly exit_code: number;
  readonly wall_clock_ns: number;
  readonly phase_timings?: NativePhaseTimingsRecord | null;
  readonly egress_bytes: number;
  readonly egress_dropped_bytes: number;
  readonly extensions: readonly ReceiptExtension[];
}

export interface NativeTrapInfoRecord {
  readonly pid: number;
  readonly ppid: number;
  readonly binary_path: string;
  readonly binary_blake3: string;
  readonly argv: string[];
  readonly cwd: string;
  readonly syscall_nr?: number;
  readonly prot_flags?: number;
  readonly memfd_flags?: number;
}

export function fromNativeGuestClock(
  raw: NativeGuestClockRecord,
): GuestClock {
  return {
    absoluteWallClockNs: raw.absolute_wall_clock_ns,
    monotonicOffsetNs: raw.monotonic_offset_ns,
    timezone: raw.timezone,
    bootParams: raw.boot_params,
  };
}

export function fromNativeSnapshotDescriptor(
  raw: NativeSnapshotRecord,
): SnapshotDescriptor {
  return {
    id: raw.id,
    dirtyBytes: raw.dirty_bytes,
    createdAtNs: raw.created_at_ns,
    parentDigest: raw.parent_digest ?? null,
    guestClock: raw.guest_clock ? fromNativeGuestClock(raw.guest_clock) : null,
  };
}

export function fromNativeCausalDelta(
  raw: NativeCausalDeltaRecord,
): CausalDelta {
  return {
    memoryDiffBytes: raw.memory_diff_bytes,
    modifiedInodes: raw.modified_inodes,
    spawnedPids: raw.spawned_pids,
    egressBytes: raw.egress_bytes,
    storageDeltaRoot: raw.storage_delta_root,
    receiptLeafHash: raw.receipt_leaf_hash,
  };
}

export function fromNativePhaseTimings(
  raw: NativePhaseTimingsRecord,
): PhaseTimings {
  return {
    trapIngressNs: raw.trap_ingress_ns,
    ffiHopNs: raw.ffi_hop_ns,
    policyNs: raw.policy_ns,
    verdictEgressNs: raw.verdict_egress_ns,
  };
}

export function fromNativeExitKind(raw: NativeExitKindRecord): ExitKind {
  if (raw === "Normal") return { kind: "normal" };
  if (raw === "DirtyShutdown") return { kind: "dirty-shutdown" };
  if (raw === "HardKilled") return { kind: "hard-killed" };
  if (typeof raw === "object" && "Signal" in raw) {
    return { kind: "signal", signal: raw.Signal };
  }
  if (typeof raw === "object" && "KernelPanic" in raw) {
    return { kind: "kernel-panic", message: raw.KernelPanic };
  }
  return { kind: "hypervisor-fault", message: raw.HypervisorFault };
}

export function fromNativeExecutionReceipt(
  raw: NativeExecutionReceiptRecord,
): ExecutionReceipt {
  return {
    pid: raw.pid,
    binaryPath: raw.binary_path,
    binaryBlake3: raw.binary_blake3,
    exitKind: fromNativeExitKind(raw.exit_kind),
    correlationToken: raw.correlation_token ?? null,
    dirtyBytes: raw.dirty_bytes,
    syscallCount: raw.syscall_count ?? null,
    exitCode: raw.exit_code,
    wallClockNs: raw.wall_clock_ns,
    phaseTimings: raw.phase_timings
      ? fromNativePhaseTimings(raw.phase_timings)
      : null,
    egressBytes: raw.egress_bytes,
    egressDroppedBytes: raw.egress_dropped_bytes,
    extensions: raw.extensions,
  };
}

export function fromNativeTrapInfo(raw: NativeTrapInfoRecord): TrapInfo {
  return {
    pid: raw.pid,
    ppid: raw.ppid,
    binaryPath: raw.binary_path,
    binaryBlake3: raw.binary_blake3,
    argv: raw.argv,
    cwd: raw.cwd,
    syscallNr: raw.syscall_nr,
    protFlags: raw.prot_flags,
    memfdFlags: raw.memfd_flags,
  };
}
