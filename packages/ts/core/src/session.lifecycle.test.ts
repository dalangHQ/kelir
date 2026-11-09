// see: cans/03-sdk-architecture.md#TypeScript-SDK

import {
  Session,
  relaySocketPath,
  type NativeSessionHandle,
} from "./session.lifecycle.js";
import { EConfig } from "./errors.sdk.js";
import type {
  CausalDelta,
  NativeCausalDeltaRecord,
  NativeSnapshotRecord,
  SnapshotDescriptor,
  SpawnOptions,
  SpawnResult,
} from "./types.core.js";

declare const process: {
  env: Record<string, string | undefined>;
};

interface RecordedCall {
  readonly method: string;
  readonly args: readonly unknown[];
}

type NativeTestHandle = NativeSessionHandle & {
  readonly calls: readonly RecordedCall[];
};

function check(condition: boolean, label: string): void {
  if (!condition) {
    throw new Error(`session.lifecycle.test: ${label}`);
  }
}

function checkThrows(
  run: () => unknown,
  messagePart: string,
  label: string,
): void {
  try {
    run();
  } catch (error) {
    check(
      error instanceof EConfig && error.message.includes(messagePart),
      `${label} (EConfig message)`,
    );
    return;
  }
  throw new Error(
    `session.lifecycle.test: ${label} (expected EConfig throw)`,
  );
}

async function checkRejects(
  promise: Promise<unknown>,
  messagePart: string,
  label: string,
): Promise<void> {
  try {
    await promise;
  } catch (error) {
    check(
      error instanceof EConfig && error.message.includes(messagePart),
      `${label} (EConfig message)`,
    );
    return;
  }
  throw new Error(
    `session.lifecycle.test: ${label} (expected EConfig rejection)`,
  );
}

function callsOf(
  native: NativeTestHandle,
  method: string,
): readonly RecordedCall[] {
  return native.calls.filter((call) => call.method === method);
}

function withHome<T>(home: string | undefined, run: () => T): T {
  const previous = process.env.HOME;
  if (home === undefined) {
    delete process.env.HOME;
  } else {
    process.env.HOME = home;
  }
  try {
    return run();
  } finally {
    if (previous === undefined) {
      delete process.env.HOME;
    } else {
      process.env.HOME = previous;
    }
  }
}

const NATIVE_SNAPSHOT: NativeSnapshotRecord = {
  id: "snap-0001",
  dirty_bytes: 4096,
  created_at_ns: 1_700_000_000_000_000_000,
  parent_digest: "parent-digest",
  guest_clock: {
    absolute_wall_clock_ns: 1_700_000_000_000_000_000,
    monotonic_offset_ns: 42,
    timezone: "UTC",
    boot_params: "console=ttyS0",
  },
};

function fakeNativeSession(options?: {
  readonly omitTerminal?: boolean;
  readonly omitCheckpoint?: boolean;
  readonly omitHost?: boolean;
}): NativeTestHandle {
  const calls: RecordedCall[] = [];
  const record = (method: string, ...args: readonly unknown[]): void => {
    calls.push({ method, args });
  };
  const native: NativeTestHandle = {
    calls,
    driverName: "qemu-microvm",
    bootMs: 1337,
    spawn: async (
      binary: string,
      argv: string[],
      spawnOptions?: SpawnOptions,
    ): Promise<SpawnResult> => {
      record("spawn", binary, argv, spawnOptions);
      return { exitCode: 0, stdout: `ran ${binary}` };
    },
    execInGuest: async (
      binary: string,
      argv: string[],
      spawnOptions?: SpawnOptions,
    ): Promise<SpawnResult> => {
      record("execInGuest", binary, argv, spawnOptions);
      return { exitCode: 3, stderr: `guest ${binary}` };
    },
    suspend: async (path: string): Promise<void> => {
      record("suspend", path);
    },
    resume: async (path: string): Promise<void> => {
      record("resume", path);
    },
    rollback: async (): Promise<void> => {
      record("rollback");
    },
    stop: async (): Promise<void> => {
      record("stop");
    },
    terminate: async (): Promise<void> => {
      record("terminate");
    },
    console: async (): Promise<void> => {
      record("console");
    },
    getCausalDeltaFromParent: async (): Promise<NativeCausalDeltaRecord> => ({
      memoryDiffBytes: 2048,
      modifiedInodes: ["/tmp/a", "/tmp/b"],
      spawnedPids: [7, 8],
      egressBytes: 99,
      storageDeltaRoot: "/var/lib/kelir/delta",
      receiptLeafHash: "leaf-hash",
    }),
    handleIngress: async function* <Req, Resp>(
      ingressOptions: {
        cwd?: string;
        handler: (req: Req) => Promise<Resp>;
      },
    ): AsyncGenerator<Resp, void, unknown> {
      record("handleIngress", ingressOptions.cwd);
      yield 3 as unknown as Resp;
      yield 4 as unknown as Resp;
    },
    vm: {
      checkpoint: async (name: string): Promise<NativeSnapshotRecord> => {
        record("vm.checkpoint", name);
        return { ...NATIVE_SNAPSHOT, id: `snap-${name}` };
      },
      rollback: async (snapshot: SnapshotDescriptor): Promise<void> => {
        record("vm.rollback", snapshot);
      },
      fork: async (): Promise<NativeSessionHandle> => {
        record("vm.fork");
        return fakeNativeSession();
      },
      suspend: async (path: string): Promise<void> => {
        record("vm.suspend", path);
      },
      resume: async (path: string): Promise<void> => {
        record("vm.resume", path);
      },
      clock: {
        skew: async (skewOptions: {
          days?: number;
          hours?: number;
          ms?: number;
        }): Promise<void> => {
          record("vm.clock.skew", skewOptions);
        },
      },
      adjustBalloon: async (megabytes: number): Promise<void> => {
        record("vm.adjustBalloon", megabytes);
      },
      setVcpus: async (count: number): Promise<void> => {
        record("vm.setVcpus", count);
      },
      mapSharedRegion: async (
        tag: string,
        sizeBytes: number,
      ): Promise<Uint8Array> => {
        record("vm.mapSharedRegion", tag, sizeBytes);
        return new Uint8Array([1, 2, 3]);
      },
      exportSnapshot: async (): Promise<Uint8Array> => {
        record("vm.exportSnapshot");
        return new Uint8Array([9, 8, 7]);
      },
      importSnapshot: async (data: Uint8Array): Promise<void> => {
        record("vm.importSnapshot", data);
      },
    },
    storage: {
      reconcile: async (): Promise<{
        filesChanged: number;
        tombstonesResolved: number;
      }> => ({ filesChanged: 3, tombstonesResolved: 1 }),
      discard: async (): Promise<void> => {
        record("storage.discard");
      },
    },
    wire: {
      forwardPort: async (
        guestPort: number,
        hostPort: number,
      ): Promise<number> => {
        record("wire.forwardPort", guestPort, hostPort);
        return hostPort;
      },
    },
    vsock: {
      channel: async (kind: number): Promise<{
        write(data: Uint8Array): Promise<void>;
        read(): Promise<Uint8Array>;
      }> => {
        record("vsock.channel", kind);
        return {
          write: async (): Promise<void> => undefined,
          read: async (): Promise<Uint8Array> => new Uint8Array([0x70]),
        };
      },
    },
    step: {
      last: (): SnapshotDescriptor => ({
        id: "snap-step",
        dirtyBytes: 0,
        createdAtNs: 5,
        parentDigest: null,
        guestClock: null,
      }),
    },
    ...(options?.omitTerminal === true
      ? {}
      : {
          terminal: {
            notice: (message: string): void => {
              record("terminal.notice", message);
            },
            resize: async (cols: number, rows: number): Promise<void> => {
              record("terminal.resize", cols, rows);
            },
            exportScreenMatrix: async (): Promise<Uint8Array> =>
              new Uint8Array([0x1b]),
            readFrom: async (
              port: number,
              offset: number,
            ): Promise<{ bytes: Uint8Array; nextOffset: number }> => {
              record("terminal.readFrom", port, offset);
              return { bytes: new Uint8Array([65]), nextOffset: offset + 1 };
            },
          },
        }),
    ...(options?.omitCheckpoint === true
      ? {}
      : {
          checkpoint: {
            create: async (label: string): Promise<NativeSnapshotRecord> => {
              record("checkpoint.create", label);
              return NATIVE_SNAPSHOT;
            },
          },
        }),
    ...(options?.omitHost === true
      ? {}
      : {
          host: {
            lsmStatus: async (): Promise<boolean> => {
              record("host.lsmStatus");
              return true;
            },
            pressureLevel: async (): Promise<number> => {
              record("host.pressureLevel");
              return 2;
            },
          },
        }),
  };
  return native;
}

// see: cans/08-lifecycle.md#Detached-lifecycle
function relaySocketPathRejectsMalformedSessionIds(): void {
  checkThrows(
    () => relaySocketPath(""),
    "not 32 hex chars",
    "relaySocketPath rejects an empty session id",
  );
  checkThrows(
    () => relaySocketPath("abc"),
    "not 32 hex chars",
    "relaySocketPath rejects a short session id",
  );
  checkThrows(
    () => relaySocketPath("z".repeat(32)),
    "not 32 hex chars",
    "relaySocketPath rejects a non-hex session id",
  );
  checkThrows(
    () => relaySocketPath("f".repeat(31)),
    "not 32 hex chars",
    "relaySocketPath rejects a 31-char session id",
  );
  checkThrows(
    () => relaySocketPath("f".repeat(33)),
    "not 32 hex chars",
    "relaySocketPath rejects a 33-char session id",
  );
}

// see: cans/08-lifecycle.md#Detached-lifecycle
function relaySocketPathFormatsTheRelaySocket(): void {
  withHome("/home/kelir", () => {
    const path = relaySocketPath("ABCDEF0123456789ABCDEF0123456789");
    check(
      path ===
        "/home/kelir/.kelir/sessions/abcdef0123456789abcdef0123456789/relay.sock",
      "relaySocketPath lowercases the id under ~/.kelir/sessions/<id>/relay.sock",
    );
  });
}

function relaySocketPathRequiresHome(): void {
  withHome(undefined, () => {
    checkThrows(
      () => relaySocketPath("f".repeat(32)),
      "HOME is not set",
      "relaySocketPath rejects when HOME is unset",
    );
  });
}

// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
function sessionSurfacesNativePlanes(): void {
  const native = fakeNativeSession();
  const session = new Session(native);
  check(
    session.driverName === "qemu-microvm",
    "driverName mirrors the native handle",
  );
  check(session.bootMs === 1337, "bootMs mirrors the native handle");
  check(session.storage === native.storage, "storage plane passes through");
  check(session.wire === native.wire, "wire plane passes through");
  check(session.vsock === native.vsock, "vsock plane passes through");
  check(session.step === native.step, "step plane passes through");
  check(
    session.terminal === native.terminal,
    "terminal plane passes through when present",
  );
  check(session.host === native.host, "host plane passes through when present");
  check(
    session.vm !== (native.vm as unknown),
    "vm plane is wrapped for record conversion",
  );
}

// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
async function sessionFallsBackOnMissingPlanes(): Promise<void> {
  const session = new Session(
    fakeNativeSession({
      omitTerminal: true,
      omitCheckpoint: true,
      omitHost: true,
    }),
  );
  check(
    session.driverName === "qemu-microvm",
    "scalar planes still surface without optional planes",
  );
  checkThrows(
    () => session.terminal.notice("boot"),
    "missing the terminal plane",
    "terminal.notice throws without a native terminal",
  );
  await checkRejects(
    session.terminal.resize(80, 24),
    "missing the terminal plane",
    "terminal.resize rejects without a native terminal",
  );
  await checkRejects(
    session.terminal.exportScreenMatrix(),
    "missing the terminal plane",
    "terminal.exportScreenMatrix rejects without a native terminal",
  );
  await checkRejects(
    session.checkpoint.create("step"),
    "missing the checkpoint plane",
    "checkpoint.create rejects without a native checkpoint plane",
  );
  await checkRejects(
    session.host.lsmStatus(),
    "missing the host plane",
    "host.lsmStatus rejects without a native host plane",
  );
  await checkRejects(
    session.host.pressureLevel(),
    "missing the host plane",
    "host.pressureLevel rejects without a native host plane",
  );
}

async function sessionConvertsCheckpointRecords(): Promise<void> {
  const native = fakeNativeSession();
  const session = new Session(native);
  const descriptor = await session.checkpoint.create("step-1");
  check(descriptor.id === "snap-0001", "checkpoint.create maps the record id");
  check(
    descriptor.dirtyBytes === 4096,
    "checkpoint.create maps dirty_bytes to dirtyBytes",
  );
  check(
    descriptor.createdAtNs === 1_700_000_000_000_000_000,
    "checkpoint.create maps created_at_ns to createdAtNs",
  );
  check(
    descriptor.parentDigest === "parent-digest",
    "checkpoint.create maps parent_digest to parentDigest",
  );
  check(
    descriptor.guestClock?.absoluteWallClockNs ===
      1_700_000_000_000_000_000,
    "checkpoint.create maps the guest clock wall time",
  );
  check(
    descriptor.guestClock?.monotonicOffsetNs === 42,
    "checkpoint.create maps the guest clock monotonic offset",
  );
  check(
    descriptor.guestClock?.timezone === "UTC",
    "checkpoint.create maps the guest clock timezone",
  );
  check(
    descriptor.guestClock?.bootParams === "console=ttyS0",
    "checkpoint.create maps the guest clock boot params",
  );
  const checkpointCalls = callsOf(native, "checkpoint.create");
  check(
    checkpointCalls.length === 1 && checkpointCalls[0].args[0] === "step-1",
    "checkpoint.create forwards the label to the native plane",
  );
}

async function sessionWrapsVmPlane(): Promise<void> {
  const native = fakeNativeSession();
  const session = new Session(native);
  const bootDescriptor = await session.vm.checkpoint("boot");
  check(
    bootDescriptor.id === "snap-boot",
    "vm.checkpoint maps the native snapshot record id",
  );
  check(
    bootDescriptor.dirtyBytes === 4096,
    "vm.checkpoint maps dirty_bytes to dirtyBytes",
  );
  check(
    callsOf(native, "vm.checkpoint")[0].args[0] === "boot",
    "vm.checkpoint forwards the snapshot name",
  );
  const forked = await session.vm.fork();
  check(
    forked instanceof Session,
    "vm.fork returns a Session wrapping the forked native handle",
  );
  check(
    forked.driverName === "qemu-microvm",
    "vm.fork preserves the forked driver name",
  );
  const descriptor: SnapshotDescriptor = {
    id: "snap-rollback",
    dirtyBytes: 10,
    createdAtNs: 7,
    parentDigest: null,
    guestClock: null,
  };
  await session.vm.rollback(descriptor);
  const vmRollbackCalls = callsOf(native, "vm.rollback");
  check(
    vmRollbackCalls.length === 1 && vmRollbackCalls[0].args[0] === descriptor,
    "vm.rollback forwards the descriptor by identity",
  );
  const skewOptions = { hours: 2 };
  await session.vm.clock.skew(skewOptions);
  check(
    callsOf(native, "vm.clock.skew")[0].args[0] === skewOptions,
    "vm.clock.skew forwards the options by identity",
  );
  await session.vm.adjustBalloon(256);
  check(
    callsOf(native, "vm.adjustBalloon")[0].args[0] === 256,
    "vm.adjustBalloon forwards the megabytes",
  );
  await session.vm.setVcpus(4);
  check(
    callsOf(native, "vm.setVcpus")[0].args[0] === 4,
    "vm.setVcpus forwards the count",
  );
  const shared = await session.vm.mapSharedRegion("tags.v1", 4096);
  check(
    shared.length === 3 && shared[0] === 1,
    "vm.mapSharedRegion returns the native bytes",
  );
  const exported = await session.vm.exportSnapshot();
  check(
    exported.length === 3 && exported[0] === 9,
    "vm.exportSnapshot returns the native bytes",
  );
  const payload = new Uint8Array([5, 6]);
  await session.vm.importSnapshot(payload);
  check(
    callsOf(native, "vm.importSnapshot")[0].args[0] === payload,
    "vm.importSnapshot forwards the payload by identity",
  );
  await session.vm.suspend("/vm/snap");
  check(
    callsOf(native, "vm.suspend")[0].args[0] === "/vm/snap",
    "vm.suspend forwards the path",
  );
  await session.vm.resume("/vm/snap");
  check(
    callsOf(native, "vm.resume")[0].args[0] === "/vm/snap",
    "vm.resume forwards the path",
  );
}

async function sessionDelegatesLifecycleCalls(): Promise<void> {
  const native = fakeNativeSession();
  const session = new Session(native);
  const argv = ["/etc/hostname"];
  const spawnOptions = { cwd: "/" };
  const spawnResult = await session.spawn("cat", argv, spawnOptions);
  check(spawnResult.exitCode === 0, "spawn returns the native exit code");
  check(
    spawnResult.stdout === "ran cat",
    "spawn surfaces the native stdout",
  );
  const spawnCalls = callsOf(native, "spawn");
  check(spawnCalls.length === 1, "spawn reaches the native handle once");
  check(spawnCalls[0].args[0] === "cat", "spawn forwards the binary");
  check(spawnCalls[0].args[1] === argv, "spawn forwards argv by identity");
  check(
    spawnCalls[0].args[2] === spawnOptions,
    "spawn forwards spawn options by identity",
  );
  const guestResult = await session.execInGuest("ls", ["/workspace"]);
  check(guestResult.exitCode === 3, "execInGuest returns the native exit code");
  check(
    guestResult.stderr === "guest ls",
    "execInGuest surfaces the native stderr",
  );
  await session.suspend("/tmp/snapshot.snap");
  check(
    callsOf(native, "suspend")[0].args[0] === "/tmp/snapshot.snap",
    "suspend forwards the snapshot path",
  );
  await session.resume("/tmp/snapshot.snap");
  check(
    callsOf(native, "resume")[0].args[0] === "/tmp/snapshot.snap",
    "resume forwards the snapshot path",
  );
  await session.rollback();
  check(
    callsOf(native, "rollback").length === 1,
    "bare rollback reaches the native session rollback",
  );
  check(
    callsOf(native, "vm.rollback").length === 0,
    "bare rollback bypasses the vm plane",
  );
  const descriptor: SnapshotDescriptor = {
    id: "snap-routed",
    dirtyBytes: 12,
    createdAtNs: 9,
    parentDigest: null,
    guestClock: null,
  };
  await session.rollback(descriptor);
  const vmRollbackCalls = callsOf(native, "vm.rollback");
  check(
    vmRollbackCalls.length === 1 && vmRollbackCalls[0].args[0] === descriptor,
    "rollback with a descriptor routes to vm.rollback",
  );
  check(
    callsOf(native, "rollback").length === 1,
    "rollback with a descriptor bypasses the native session rollback",
  );
  await session.stop();
  check(callsOf(native, "stop").length === 1, "stop delegates to native");
  await session.terminate();
  check(
    callsOf(native, "terminate").length === 1,
    "terminate delegates to native",
  );
  await session.console();
  check(
    callsOf(native, "console").length === 1,
    "console delegates to native",
  );
  const delta: CausalDelta = await session.getCausalDeltaFromParent();
  check(
    delta.memoryDiffBytes === 2048,
    "getCausalDeltaFromParent maps memoryDiffBytes",
  );
  check(
    delta.modifiedInodes.length === 2 &&
      delta.modifiedInodes[0] === "/tmp/a" &&
      delta.modifiedInodes[1] === "/tmp/b",
    "getCausalDeltaFromParent maps modifiedInodes",
  );
  check(
    delta.spawnedPids.length === 2 && delta.spawnedPids[1] === 8,
    "getCausalDeltaFromParent maps spawnedPids",
  );
  check(
    delta.egressBytes === 99,
    "getCausalDeltaFromParent maps egressBytes",
  );
  check(
    delta.storageDeltaRoot === "/var/lib/kelir/delta",
    "getCausalDeltaFromParent maps storageDeltaRoot",
  );
  check(
    delta.receiptLeafHash === "leaf-hash",
    "getCausalDeltaFromParent maps receiptLeafHash",
  );
  const yielded: unknown[] = [];
  for await (const value of session.handleIngress<number, number>({
    cwd: "/workspace",
    handler: async (req: number): Promise<number> => req + 1,
  })) {
    yielded.push(value);
  }
  check(
    yielded.length === 2 && yielded[0] === 3 && yielded[1] === 4,
    "handleIngress yields every native generator value in order",
  );
  check(
    callsOf(native, "handleIngress")[0].args[0] === "/workspace",
    "handleIngress forwards options to the native generator",
  );
}

// see: cans/08-lifecycle.md#Detached-lifecycle
async function sessionAttachRejectsUntilL3(): Promise<void> {
  await checkRejects(
    Session.attach("nope"),
    "not 32 hex chars",
    "attach rejects a malformed session id before any relay work",
  );
  await withHome("/home/kelir", async (): Promise<void> => {
    await checkRejects(
      Session.attach("f".repeat(32)),
      "TODO: Session.attach",
      "attach rejects with the L3 TODO for a valid session id",
    );
  });
}

async function main(): Promise<void> {
  relaySocketPathRejectsMalformedSessionIds();
  relaySocketPathFormatsTheRelaySocket();
  relaySocketPathRequiresHome();
  sessionSurfacesNativePlanes();
  await sessionFallsBackOnMissingPlanes();
  await sessionConvertsCheckpointRecords();
  await sessionWrapsVmPlane();
  await sessionDelegatesLifecycleCalls();
  await sessionAttachRejectsUntilL3();
}

main().catch((error: unknown): never => {
  throw error;
});
