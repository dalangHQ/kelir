// see: cans/03-sdk-architecture.md#TypeScript-SDK

import type { Session } from "./session.js";
import type {
  CausalDelta,
  CustomActionPayload,
  ExecveMutation,
  ExecutionReceipt,
  SnapshotDescriptor,
  TrapInfo,
} from "./types.js";
import type { SuspendUntil, Verdict, VetoEvent } from "./verdict.js";

export interface VmHandle {
  checkpoint(name: string): Promise<SnapshotDescriptor>;
  rollback(snapshot: SnapshotDescriptor): Promise<void>;
  fork(): Promise<Session>;
  suspend(path: string): Promise<void>;
  resume(path: string): Promise<void>;
  readonly clock: {
    skew(options: { days?: number; hours?: number; ms?: number }): Promise<void>;
  };
  adjust_balloon(megabytes: number): Promise<void>;
  adjustBalloon(megabytes: number): Promise<void>;
}

export interface LedgerHandle {
  flush(): Promise<void>;
  countDenials(ppid: number): Promise<number>;
  finalize(options: {
    includeDelta?: CausalDelta | null;
    includeMerkleRoot?: boolean;
  }): Promise<{ merkleRoot: string; signedJws: string }>;
  blake3OfProducingProcess(path: string): Promise<string>;
}

export interface WireHandle {
  forwardPort(guestPort: number, hostPort: number): Promise<void>;
}

export interface StorageHandle {
  reconcile(options: {
    cwd: string;
    atomicStaging?: boolean;
    fsync?: boolean;
  }): Promise<{ filesChanged: number; tombstonesResolved: number }>;
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

export interface LifecycleContext {
  setWatchdog(options: { timeoutMs: number; action: string }): void;
  registerVeto(event: VetoEvent, predicate: () => boolean): void;
}

export interface TrapContext {
  readonly trap: TrapInfo;
  readonly vm: VmHandle;
  readonly fs: FsHandle;
  readonly step: StepHandle;
  readonly terminal: TerminalHandle;
  readonly policy: PolicyHandle;
  readonly ledger: LedgerHandle;
  readonly mesh: MeshHandle;
  readonly chaos: ChaosHandle;
  receipt(): Promise<ExecutionReceipt>;
  pass(): Verdict;
  drop(errno: string, reason?: string): Verdict;
  patch(mutation: ExecveMutation | CustomActionPayload): Verdict;
  suspend(until: SuspendUntil): Verdict;
  abort(reason?: string): Verdict;
}
