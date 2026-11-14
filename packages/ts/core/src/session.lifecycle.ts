// see: cans/03-sdk-architecture.md#TypeScript-SDK

import type {
  CausalDelta,
  NativeCausalDeltaRecord,
  NativeSnapshotDescriptorRecord,
  SnapshotDescriptor,
  SpawnOptions,
  SpawnResult,
} from "./types.core.js";
import {
  fromNativeCausalDelta,
  fromNativeSnapshotDescriptor,
} from "./types.core.js";
import type {
  CheckpointHandle,
  HostHandle,
  StepHandle,
  StorageHandle,
  TerminalHandle,
  VmHandle,
  VsockHandle,
  WireHandle,
} from "./context.lifecycle.types.js";
import type { SuspendReason } from "./verdict.protocol.js";
import { EConfig } from "./errors.sdk.js";
import { loadNativeAddon } from "./fusion.plugin.js";

// see: cans/08-lifecycle.md#Detached-lifecycle
export async function relaySocketPath(sessionId: string): Promise<string> {
  const addon = await loadNativeAddon();
  try {
    return addon.relaySocketPath(sessionId);
  } catch (error) {
    throw new EConfig(
      error instanceof Error ? error.message : String(error),
    );
  }
}

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
  handleIngress<Req, Resp>(options: {
    cwd?: string;
    handler: (req: Req) => Promise<Resp>;
  }): AsyncGenerator<Resp, void, unknown>;
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
  readonly vsock: VsockHandle;
  readonly step: StepHandle;
  private readonly native: NativeSessionHandle;
  private suspendedReasonValue: SuspendReason | undefined;

  constructor(native: NativeSessionHandle) {
    // TODO (L3) [bt_04_napi_boot_before_fuse_hooks_fails_closed]: brand the native handle at the verified addon boundary so only addon-produced handles construct a Session.; slop: Session constructed from an unverified handle, boot check bypassed
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
    this.vsock = native.vsock;
    this.step = native.step;
    const vm = native.vm;
    this.vm = {
      checkpoint: async (name: string): Promise<SnapshotDescriptor> =>
        fromNativeSnapshotDescriptor(await vm.checkpoint(name)),
      rollback: (snapshot: SnapshotDescriptor): Promise<void> =>
        vm.rollback(snapshot),
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

  // see: cans/03-sdk-architecture.md#Sandbox-high-level-facade
  get sessionDir(): string {
    const sessionDir = this.native.sessionDir;
    if (sessionDir === undefined) {
      throw new EConfig(
        "kelir native bridge is missing the sessionDir member on the session handle (A11)",
      );
    }
    return sessionDir;
  }

  // see: cans/03-sdk-architecture.md#Service-lookup-across-languages
  cap<T>(plugin: string): T | undefined {
    return this.native.pluginContexts?.find(
      (entry) => entry.name === plugin,
    )?.ctx as T | undefined;
  }

  // see: cans/03-sdk-architecture.md#Service-lookup-across-languages
  plane<T>(name: string): T | undefined {
    switch (name) {
      case "vm":
        return this.vm as T;
      case "terminal":
        return this.terminal as T;
      case "storage":
        return this.storage as T;
      case "wire":
        return this.wire as T;
      case "checkpoint":
        return this.checkpoint as T;
      case "host":
        return this.host as T;
      case "vsock":
        return this.vsock as T;
      case "step":
        return this.step as T;
      default:
        return undefined;
    }
  }

  // see: cans/08-lifecycle.md#CoW-fork
  async fork(): Promise<Session> {
    return new Session(await this.native.fork());
  }

  // see: cans/08-lifecycle.md#Clock-skew
  async skewClock(options: {
    days?: number;
    hours?: number;
    ms?: number;
  }): Promise<void> {
    return this.vm.clock.skew(options);
  }

  async execInGuest(
    binary: string,
    argv: string[],
    options?: SpawnOptions,
  ): Promise<SpawnResult> {
    return this.native.execInGuest(binary, argv, options);
  }

  // see: cans/08-lifecycle.md#Suspend-and-resume
  get suspendedReason(): SuspendReason | undefined {
    return this.suspendedReasonValue;
  }

  async suspend(reason: SuspendReason): Promise<void> {
    await this.native.suspend(reason);
    this.suspendedReasonValue = reason;
  }

  async resume(): Promise<void> {
    await this.native.resume();
    this.suspendedReasonValue = undefined;
  }

  async rollback(snapshot?: SnapshotDescriptor): Promise<void> {
    if (snapshot !== undefined) {
      return this.vm.rollback(snapshot);
    }
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

  // see: cans/08-lifecycle.md#Detached-lifecycle
  static async attach(sessionId: string): Promise<Session> {
    const relay = await relaySocketPath(sessionId);
    void relay;
    // TODO (L3) [lc_15_detached_session_reconnect_relay]: connect to relay.sock, verify the catalog lease is live, and rebuild the session from the runner-owned state.; slop: attach skips lease check, binds dead relay.sock
    throw new EConfig("TODO: Session.attach (L3)");
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
}
