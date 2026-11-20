// see: cans/03-sdk-architecture.md#TypeScript-SDK

import type {
  NativeCausalDeltaRecord,
  NativeSnapshotDescriptorRecord,
  SnapshotDescriptor,
  SpawnOptions,
  SpawnResult,
} from "./types.core.js";
import type {
  PidLineage,
  ShmMapping,
  StepHandle,
  StorageHandle,
  VsockHandle,
  WireHandle,
} from "./context.lifecycle.types.js";
import type { SuspendReason } from "./verdict.protocol.js";

// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
export interface NativeTerminalHandle {
  notice(message: string): void;
  resize(cols: number, rows: number): Promise<void>;
  exportScreenMatrix(): Promise<Uint8Array>;
  readFrom(
    port: number,
    offset: number,
  ): Promise<{ bytes: Uint8Array; nextOffset: number }>;
}

// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
export interface NativeCheckpointHandle {
  create(label: string): Promise<NativeSnapshotDescriptorRecord>;
  snapshot(
    lineage: NativePidLineageRecord,
    descriptor: NativeSnapshotDescriptorRecord,
  ): Promise<number>;
  fork(nodeId: number, lineage: NativePidLineageRecord): Promise<void>;
  branchFrom(nodeId: number, lineage: NativePidLineageRecord): Promise<void>;
  rollbackNode(
    nodeId: number,
    guestMonotonicNs?: number,
  ): Promise<NativeSnapshotDescriptorRecord>;
  compact(lineage: NativePidLineageRecord): Promise<number>;
  rollback(snapshotId: string): Promise<void>;
}

// see: cans/09-snapshots.md#Groups-and-lineage
export interface NativePidLineageRecord {
  readonly root_pid: number;
  readonly incarnation_id: Uint8Array;
}

export function toNativePidLineage(
  lineage: PidLineage,
): NativePidLineageRecord {
  return {
    root_pid: lineage.rootPid,
    incarnation_id: lineage.incarnationId,
  };
}

export function toNativeSnapshotDescriptor(
  descriptor: SnapshotDescriptor,
): NativeSnapshotDescriptorRecord {
  return {
    id: descriptor.id,
    dirty_bytes: descriptor.dirtyBytes,
    created_at_ns: descriptor.createdAtNs,
    parent_digest: descriptor.parentDigest ?? null,
    guest_clock: descriptor.guestClock
      ? {
          absolute_wall_clock_ns: descriptor.guestClock.absoluteWallClockNs,
          monotonic_offset_ns: descriptor.guestClock.monotonicOffsetNs,
          timezone: descriptor.guestClock.timezone,
          boot_params: descriptor.guestClock.bootParams,
        }
      : null,
  };
}

// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
export interface NativeHostHandle {
  lsmStatus(): Promise<boolean>;
  pressureLevel(): Promise<number>;
}

// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
export interface NativeVmHandle {
  checkpoint(name: string): Promise<NativeSnapshotDescriptorRecord>;
  rollback(snapshot: SnapshotDescriptor): Promise<void>;
  suspend(path: string): Promise<void>;
  resume(path: string): Promise<void>;
  readonly clock: {
    skew(options: {
      days?: number;
      hours?: number;
      ms?: number;
    }): Promise<void>;
  };
  adjustBalloon(megabytes: number): Promise<void>;
  setVcpus(count: number): Promise<void>;
  mapSharedRegion(tag: string, sizeBytes: number): Promise<Uint8Array>;
  exportSnapshot(): Promise<Uint8Array>;
  importSnapshot(data: Uint8Array): Promise<void>;
}

// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
export interface NativeSessionHandle {
  readonly driverName: string;
  readonly bootMs: number;
  readonly vm: NativeVmHandle;
  readonly terminal?: NativeTerminalHandle;
  readonly storage: StorageHandle;
  readonly wire: WireHandle;
  readonly checkpoint?: NativeCheckpointHandle;
  readonly host?: NativeHostHandle;
  readonly vsock: VsockHandle;
  readonly step: StepHandle;
  spawn(
    binary: string,
    argv: string[],
    options?: SpawnOptions,
  ): Promise<SpawnResult>;
  execInGuest(
    binary: string,
    argv: string[],
    options?: SpawnOptions,
  ): Promise<SpawnResult>;
  fork(): Promise<NativeSessionHandle>;
  suspend(reason: SuspendReason): Promise<void>;
  resume(): Promise<void>;
  rollback(): Promise<void>;
  stop(): Promise<void>;
  terminate(): Promise<void>;
  console(): Promise<void>;
  getCausalDeltaFromParent(): Promise<NativeCausalDeltaRecord>;
  // see: cans/03-sdk-architecture.md#Sandbox-high-level-facade
  readonly sessionDir?: string;
  // see: cans/03-sdk-architecture.md#Service-lookup-across-languages
  readonly pluginContexts?: readonly {
    readonly name: string;
    readonly ctx: unknown;
  }[];
  handleIngress<Req, Resp>(
    port: number,
  ): AsyncGenerator<IngressStream<Req, Resp>, void, unknown>;
  shm(region: string): Promise<ShmMapping>;
}

// see: cans/plugins/wire-ingress.md#Plugin-wire-ingress
export interface IngressStream<Req, Resp> {
  readonly request: Req;
  respond(response: Resp): Promise<void>;
}
