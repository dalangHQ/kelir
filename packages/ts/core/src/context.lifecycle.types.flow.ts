// see: cans/03-sdk-architecture.md#TypeScript-SDK

import type {
  CausalDelta,
  ExecutionReceipt,
  PatchInput,
  SnapshotDescriptor,
  SpawnResult,
  TrapInfo,
  WatchdogAction,
} from "./types.core.flow.js";
import type { SuspendUntil, Verdict, VetoEvent } from "./verdict.protocol.flow.js";

export interface VmHandle {
  checkpoint(name: string): Promise<SnapshotDescriptor>;
  rollback(snapshot: SnapshotDescriptor): Promise<void>;
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
  finalize(options: {
    includeDelta?: CausalDelta | null;
    includeMerkleRoot?: boolean;
  }): Promise<{ merkleRoot: string; signedJws: string }>;
  blake3OfProducingProcess(path: string): Promise<string>;
  getCausalDeltaFromParent(): Promise<CausalDelta>;
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

// see: cans/09-snapshots.md#Groups-and-lineage
export interface PidLineage {
  readonly rootPid: number;
  readonly incarnationId: Uint8Array;
}

// see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
export interface ShmMapping {
  readonly region: string;
}

export interface CheckpointHandle {
  create(label: string): Promise<SnapshotDescriptor>;
  snapshot(lineage: PidLineage, descriptor: SnapshotDescriptor): Promise<number>;
  fork(nodeId: number, lineage: PidLineage): Promise<void>;
  branchFrom(nodeId: number, lineage: PidLineage): Promise<void>;
  rollbackNode(nodeId: number, guestMonotonicNs?: number): Promise<SnapshotDescriptor>;
  compact(lineage: PidLineage): Promise<number>;
  rollback(snapshotId: string): Promise<void>;
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
  // L3-GAP: [protocol_mux]: awaitWinner stays native-pending until the mesh winner-election wait lands per cans/plugins/mesh.md#CoW-swarm-mesh-federation.; slop: no listener wait, winning shard never awaited, election never settles
  awaitWinner(timeoutMs: number): Promise<number>;
  broadcastWinner(shardId: number): Promise<void>;
}

// see: cans/plugins/container.md#Plugin-container
export interface ContainerHandle {
  ingestImage(path: string): Promise<string>;
}

// see: cans/plugins/db-proxy.md#Plugin-db-proxy
export interface DbProxyHandle {
  readonly rewriteTarget: string;
}

// see: cans/plugins/db-proxy.md#SQL-receipt-extension
export interface NetL7SqlExt {
  readonly statements: number;
  readonly distinctShapes: number;
}

// see: cans/plugins/db-proxy.md#SQL-receipt-extension
export function netL7Sql(receipt: ExecutionReceipt): NetL7SqlExt | null {
  const entry = receipt.extensions.find(
    (extension) => extension.namespace === "net:l7:sql",
  );
  if (!entry || entry.payload.length !== 8) return null;
  const view = new DataView(
    entry.payload.buffer,
    entry.payload.byteOffset,
    entry.payload.byteLength,
  );
  return {
    statements: view.getUint32(0, true),
    distinctShapes: view.getUint32(4, true),
  };
}

// see: cans/plugins/transaction.md#Plugin-transaction
export interface Savepoint {
  readonly id: number;
}

// see: cans/plugins/transaction.md#Plugin-transaction
export interface Tx {
  savepoint(): Promise<Savepoint>;
  rollbackToSavepoint(savepoint: Savepoint): Promise<void>;
  commit(): Promise<void>;
}

export interface TransactionHandle {
  scope(
    name: string,
    options: {
      readonly paths: readonly string[];
      readonly schemas: readonly string[];
    },
  ): Promise<Tx>;
}

export interface ChaosHandle {
  readonly faults?: readonly string[];
}

export interface LifecycleContext {
  readonly vm: VmHandle;
  cap<T>(name: string): T | undefined;
  execInGuest(argv: readonly string[]): Promise<SpawnResult>;
  setWatchdog(options: { timeoutMs: number; action: WatchdogAction }): void;
  registerVeto(event: VetoEvent, predicate: () => boolean): void;
}

export interface TrapContext {
  readonly trap: TrapInfo;
  readonly vm: VmHandle;
  readonly checkpoint: CheckpointHandle;
  readonly fs: FsHandle;
  readonly step: StepHandle;
  readonly terminal: TerminalHandle;
  receipt(): Promise<ExecutionReceipt>;
  pass(): Verdict;
  drop(errno: string, reason?: string): Verdict;
  patch(mutation: PatchInput): Verdict;
  suspend(until: SuspendUntil): Verdict;
  abort(reason?: string): Verdict;
  // TODO (L3) [governance_tools]: verify the presented operator lease against the pinned ed25519 pubkey once the native handler context carries lease state; slop: operator lease never verified against the pinned pubkey; presence-only check.
  hasValidOperatorLease(): boolean;
  cap<T>(name: string): T | undefined;
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
export type HandlerContext<Ctx, Params> = TrapContext &
  Ctx & {
    readonly params: Params;
    cap<K extends keyof Ctx & string>(name: K): Ctx[K] | undefined;
    cap<T>(name: string): T | undefined;
  };
