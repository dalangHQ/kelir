// see: cans/03-sdk-architecture.md#TypeScript-SDK

import type {
  CausalDelta,
  SnapshotDescriptor,
  SpawnOptions,
  SpawnResult,
} from "./types.js";
import type {
  LedgerHandle,
  PolicyHandle,
  StepHandle,
  StorageHandle,
  VmHandle,
  VsockHandle,
  WireHandle,
} from "./context.js";

// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
export interface NativeVmHandle {
  checkpoint(name: string): Promise<SnapshotDescriptor>;
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
  adjust_balloon(megabytes: number): Promise<void>;
  adjustBalloon(megabytes: number): Promise<void>;
}

// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
export interface NativeSessionHandle {
  readonly driverName: string;
  readonly bootMs: number;
  readonly vm: NativeVmHandle;
  readonly wire: WireHandle;
  readonly storage: StorageHandle;
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
  getCausalDeltaFromParent(): Promise<CausalDelta>;
  handleIngress<Req, Resp>(options: {
    cwd?: string;
    handler: (req: Req) => Promise<Resp>;
  }): AsyncGenerator<Resp, void, unknown>;
  ctx<T>(): T;
}

export class Session {
  // see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
  readonly driverName: string;
  readonly bootMs: number;
  readonly vm: VmHandle;
  readonly wire: WireHandle;
  readonly storage: StorageHandle;
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
    this.wire = native.wire;
    this.storage = native.storage;
    this.ledger = native.ledger;
    this.vsock = native.vsock;
    this.step = native.step;
    this.policy = native.policy;
    const vm = native.vm;
    this.vm = {
      checkpoint: (name: string): Promise<SnapshotDescriptor> =>
        vm.checkpoint(name),
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
      adjust_balloon: (megabytes: number): Promise<void> =>
        vm.adjust_balloon(megabytes),
      adjustBalloon: (megabytes: number): Promise<void> =>
        vm.adjustBalloon(megabytes),
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
    return this.native.getCausalDeltaFromParent();
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
