// see: cans/03-sdk-architecture.md#TypeScript-SDK

import type { Session } from "./session.lifecycle.js";
import type {
  CausalDelta,
  CustomActionPayload,
  ExecveMutation,
  ExecutionReceipt,
  SnapshotDescriptor,
  TrapInfo,
} from "./types.core.js";
import type { SuspendUntil, Verdict, VetoEvent } from "./verdict.protocol.js";

export interface VmHandle {
  checkpoint(name: string): Promise<SnapshotDescriptor>;
  rollback(snapshot: SnapshotDescriptor): Promise<void>;
  fork(): Promise<Session>;
  suspend(path: string): Promise<void>;
  resume(path: string): Promise<void>;
  readonly clock: {
    skew(options: { days?: number; hours?: number; ms?: number }): Promise<void>;
  };
  adjustBalloon(megabytes: number): Promise<void>;
  setVcpus(count: number): Promise<void>;
  mapSharedRegion(tag: string, sizeBytes: number): Promise<Uint8Array>;
  exportSnapshot(): Promise<Uint8Array>;
  importSnapshot(data: Uint8Array): Promise<void>;
}

export interface LedgerHandle {
  flush(): Promise<void>;
  countDenials(ppid: number): Promise<number>;
  getCausalDeltaFromParent(): Promise<CausalDelta>;
  finalize(options: {
    includeDelta?: CausalDelta | null;
    includeMerkleRoot?: boolean;
  }): Promise<{ merkleRoot: string; signedJws: string }>;
  blake3OfProducingProcess(path: string): Promise<string>;
}

export interface WireHandle {
  forwardPort(guestPort: number, hostPort: number): Promise<number>;
}

export interface StorageHandle {
  reconcile(options: {
    cwd: string;
    atomicStaging?: boolean;
    fsync?: boolean;
  }): Promise<{ filesChanged: number; tombstonesResolved: number }>;
  discard(): Promise<void>;
}

export interface VsockChannel {
  write(data: Uint8Array): Promise<void>;
  read(): Promise<Uint8Array>;
}

export interface VsockHandle {
  channel(kind: number): Promise<VsockChannel>;
}

export interface StepHandle {
  last(): SnapshotDescriptor;
}

export interface FsHandle {
  warmCache(paths: string[]): Promise<void>;
}

export interface TerminalHandle {
  notice(message: string): void;
  resize(cols: number, rows: number): Promise<void>;
  exportScreenMatrix(): Promise<Uint8Array>;
  readFrom(
    port: number,
    offset: number,
  ): Promise<{ bytes: Uint8Array; nextOffset: number }>;
}

export interface CheckpointHandle {
  create(label: string): Promise<SnapshotDescriptor>;
}

export interface HostHandle {
  lsmStatus(): Promise<boolean>;
  pressureLevel(): Promise<number>;
}

export interface PolicyHandle {
  violations: number;
  currentBranch: string;
  isProtected(): boolean;
  recordViolation(): void;
}

export interface MeshHandle {
  readonly peers?: readonly string[];
}

export interface ChaosHandle {
  readonly faults?: readonly string[];
}

// see: cans/08-lifecycle.md#Session-and-context-handles
export interface LifecycleContext {
  setWatchdog(options: { timeoutMs: number; action: string }): void;
  registerVeto(event: VetoEvent, predicate: () => boolean): void;
  readonly cap?: { readonly ledger?: LedgerHandle };
}

export interface TrapContext {
  readonly trap: TrapInfo;
  readonly vm: VmHandle;
  readonly fs: FsHandle;
  readonly step: StepHandle;
  readonly terminal: TerminalHandle;
  receipt(): Promise<ExecutionReceipt>;
  pass(): Verdict;
  drop(errno: string, reason?: string): Verdict;
  patch(mutation: ExecveMutation | CustomActionPayload): Verdict;
  suspend(until: SuspendUntil): Verdict;
  abort(reason?: string): Verdict;
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
export type HandlerContext<Ctx, Params> = TrapContext & Ctx & {
  readonly params: Params;
};
