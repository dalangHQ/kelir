// see: cans/03-sdk-architecture.md#TypeScript-SDK

import type {
  CausalDelta,
  NativeCausalDeltaRecord,
  NativeSnapshotRecord,
  SnapshotDescriptor,
  SpawnOptions,
  SpawnResult,
} from "./types.js";
import {
  fromNativeCausalDelta,
  fromNativeSnapshotDescriptor,
} from "./types.js";
import type {
  CheckpointHandle,
  HostHandle,
  LedgerHandle,
  PolicyHandle,
  StepHandle,
  StorageHandle,
  TerminalHandle,
  VmHandle,
  VsockHandle,
  WireHandle,
} from "./context.js";
import { EConfig } from "./errors.js";

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
  create(label: string): Promise<NativeSnapshotRecord>;
}

// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
export interface NativeHostHandle {
  lsmStatus(): Promise<boolean>;
  pressureLevel(): Promise<number>;
}

// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
export interface NativeVmHandle {
  checkpoint(name: string): Promise<NativeSnapshotRecord>;
  rollback(snapshot: SnapshotDescriptor): Promise<void>;
  fork(): Promise<NativeSessionHandle>;
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
  readonly ledger: LedgerHandle;
  readonly vsock: VsockHandle;
  readonly step: StepHandle;
  readonly policy: PolicyHandle;
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
  suspend(path: string): Promise<void>;
  resume(path: string): Promise<void>;
  rollback(): Promise<void>;
  stop(): Promise<void>;
  terminate(): Promise<void>;
  console(): Promise<void>;
  getCausalDeltaFromParent(): Promise<NativeCausalDeltaRecord>;
  handleIngress<Req, Resp>(options: {
    cwd?: string;
    handler: (req: Req) => Promise<Resp>;
  }): AsyncGenerator<Resp, void, unknown>;
  ctx<T>(): T;
}

function missingPlaneError(plane: string): EConfig {
  return new EConfig(
    `kelir native bridge is missing the ${plane} plane (L3)`,
  );
}

function fallbackTerminal(): TerminalHandle {
  return {
    notice: (): void => {
      throw missingPlaneError("terminal");
    },
    resize: (): Promise<void> =>
      Promise.reject(missingPlaneError("terminal")),
    exportScreenMatrix: (): Promise<Uint8Array> =>
      Promise.reject(missingPlaneError("terminal")),
    readFrom: (): Promise<{ bytes: Uint8Array; nextOffset: number }> =>
      Promise.reject(missingPlaneError("terminal")),
  };
}

function fallbackCheckpoint(): CheckpointHandle {
  return {
    create: (): Promise<SnapshotDescriptor> =>
      Promise.reject(missingPlaneError("checkpoint")),
  };
}

function fallbackHost(): HostHandle {
  return {
    lsmStatus: (): Promise<boolean> =>
      Promise.reject(missingPlaneError("host")),
    pressureLevel: (): Promise<number> =>
      Promise.reject(missingPlaneError("host")),
  };
}

export class Session {
  // see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
  readonly driverName: string;
  readonly bootMs: number;
  readonly vm: VmHandle;
  readonly terminal: TerminalHandle;
  readonly storage: StorageHandle;
  readonly wire: WireHandle;
  readonly checkpoint: CheckpointHandle;
  readonly host: HostHandle;
  readonly ledger: LedgerHandle;
  readonly vsock: VsockHandle;
  readonly step: StepHandle;
  readonly policy: PolicyHandle;
  private readonly native: NativeSessionHandle;

  constructor(native: NativeSessionHandle) {
    // TODO (L3): only the NAPI addon constructs Session, from a booted native handle.
    this.native = native;
    this.driverName = native.driverName;
    this.bootMs = native.bootMs;
    this.terminal = native.terminal ?? fallbackTerminal();
    this.storage = native.storage;
    this.wire = native.wire;
    const nativeCheckpoint = native.checkpoint;
    this.checkpoint = nativeCheckpoint
      ? {
          create: async (label: string): Promise<SnapshotDescriptor> =>
            fromNativeSnapshotDescriptor(
              await nativeCheckpoint.create(label),
            ),
        }
      : fallbackCheckpoint();
    this.host = native.host ?? fallbackHost();
    this.ledger = native.ledger;
    this.vsock = native.vsock;
    this.step = native.step;
    this.policy = native.policy;
    const vm = native.vm;
    this.vm = {
      checkpoint: async (name: string): Promise<SnapshotDescriptor> =>
        fromNativeSnapshotDescriptor(await vm.checkpoint(name)),
      rollback: (snapshot: SnapshotDescriptor): Promise<void> =>
        vm.rollback(snapshot),
      fork: async (): Promise<Session> => new Session(await vm.fork()),
      suspend: (path: string): Promise<void> => vm.suspend(path),
      resume: (path: string): Promise<void> => vm.resume(path),
      clock: {
        skew: (options: {
          days?: number;
          hours?: number;
          ms?: number;
        }): Promise<void> => vm.clock.skew(options),
      },
      adjustBalloon: (megabytes: number): Promise<void> =>
        vm.adjustBalloon(megabytes),
      setVcpus: (count: number): Promise<void> => vm.setVcpus(count),
      mapSharedRegion: (
        tag: string,
        sizeBytes: number,
      ): Promise<Uint8Array> => vm.mapSharedRegion(tag, sizeBytes),
      exportSnapshot: (): Promise<Uint8Array> => vm.exportSnapshot(),
      importSnapshot: (data: Uint8Array): Promise<void> =>
        vm.importSnapshot(data),
    };
  }

  async spawn(
    binary: string,
    argv: string[],
    options?: SpawnOptions,
  ): Promise<SpawnResult> {
    return this.native.spawn(binary, argv, options);
  }

  async execInGuest(
    binary: string,
    argv: string[],
    options?: SpawnOptions,
  ): Promise<SpawnResult> {
    return this.native.execInGuest(binary, argv, options);
  }

  async suspend(path: string): Promise<void> {
    return this.native.suspend(path);
  }

  async resume(path: string): Promise<void> {
    return this.native.resume(path);
  }

  async rollback(): Promise<void> {
    return this.native.rollback();
  }

  async stop(): Promise<void> {
    return this.native.stop();
  }

  async terminate(): Promise<void> {
    return this.native.terminate();
  }

  async console(): Promise<void> {
    return this.native.console();
  }

  async getCausalDeltaFromParent(): Promise<CausalDelta> {
    return fromNativeCausalDelta(
      await this.native.getCausalDeltaFromParent(),
    );
  }

  async *handleIngress<Req, Resp>(options: {
    cwd?: string;
    handler: (req: Req) => Promise<Resp>;
  }): AsyncGenerator<Resp, void, unknown> {
    yield* this.native.handleIngress(options);
  }

  ctx<T>(): T {
    return this.native.ctx<T>();
  }
}
