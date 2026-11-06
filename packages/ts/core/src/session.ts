// see: cans/03-sdk-architecture.md#TypeScript-SDK
// Bootable session. All async methods are stubs returning zero-values;
// the native engine planes land behind them.

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

const zeroSnapshot = (name: string): SnapshotDescriptor => ({
  id: `stub:${name}`,
  name,
});

const zeroVm = (): VmHandle => ({
  checkpoint: async (name: string): Promise<SnapshotDescriptor> =>
    zeroSnapshot(name),
  rollback: async (_snapshot: SnapshotDescriptor): Promise<void> => {},
  fork: async (): Promise<Session> => new Session(),
  suspend: async (_path: string): Promise<void> => {},
  resume: async (_path: string): Promise<void> => {},
  clock: {
    skew: async (_options: {
      days?: number;
      hours?: number;
      ms?: number;
    }): Promise<void> => {},
  },
  adjust_balloon: async (_megabytes: number): Promise<void> => {},
  adjustBalloon: async (_megabytes: number): Promise<void> => {},
});

export class Session {
  // see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
  readonly driverName = "auto";
  readonly bootMs = 0;
  readonly vm: VmHandle = zeroVm();
  readonly wire: WireHandle = {
    forwardPort: async (_guestPort: number, _hostPort: number): Promise<void> => {},
  };
  readonly storage: StorageHandle = {
    reconcile: async (_options: {
      cwd: string;
      atomicStaging?: boolean;
      fsync?: boolean;
    }): Promise<{ filesChanged: number; tombstonesResolved: number }> => ({
      filesChanged: 0,
      tombstonesResolved: 0,
    }),
  };
  readonly ledger: LedgerHandle = {
    flush: async (): Promise<void> => {},
    countDenials: async (_ppid: number): Promise<number> => 0,
    finalize: async (_options: {
      includeDelta?: CausalDelta | null;
      includeMerkleRoot?: boolean;
    }): Promise<{ merkleRoot: string; signedJws: string }> => ({
      merkleRoot: "",
      signedJws: "",
    }),
    blake3OfProducingProcess: async (_path: string): Promise<string> => "",
  };
  readonly vsock: VsockHandle = {
    channel: async (_kind: number) => ({
      write: async (_data: Uint8Array): Promise<void> => {},
      read: async (): Promise<Uint8Array> => new Uint8Array(0),
    }),
  };
  readonly step: StepHandle = {
    last: (): SnapshotDescriptor => zeroSnapshot("last"),
  };
  readonly policy: PolicyHandle = {
    violations: 0,
    currentBranch: "",
    isProtected: (): boolean => false,
    recordViolation: (): void => {},
  };

  async spawn(
    _binary: string,
    _argv: string[],
    _options?: SpawnOptions,
  ): Promise<SpawnResult> {
    return { exitCode: 0 };
  }

  async execInGuest(
    _binary: string,
    _argv: string[],
    _options?: SpawnOptions,
  ): Promise<SpawnResult> {
    return { exitCode: 0 };
  }

  async suspend(_path: string): Promise<void> {}

  async resume(_path: string): Promise<void> {}

  async rollback(): Promise<void> {
    // TODO: Call native session rollback
  }

  async stop(): Promise<void> {
    // TODO: Call native session stop
  }

  async terminate(): Promise<void> {}

  async console(): Promise<void> {}

  async getCausalDeltaFromParent(): Promise<CausalDelta> {
    return {};
  }

  async *handleIngress<Req, Resp>(_options: {
    cwd?: string;
    handler: (req: Req) => Promise<Resp>;
  }): AsyncGenerator<Resp, void, unknown> {
    // Stub: no ingress arrives without a native engine.
    return;
  }

  ctx<T>(): T {
    return undefined as unknown as T;
  }
}
