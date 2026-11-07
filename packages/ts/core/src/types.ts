// see: cans/03-sdk-architecture.md#TypeScript-SDK

export interface SnapshotDescriptor {
  readonly id: string;
  readonly name: string;
}

export interface CausalDelta {
  readonly parentSnapshot?: string;
  readonly bytes?: number;
}

export interface ReceiptExtension {
  readonly namespace: string;
  readonly payload: Uint8Array;
}

export interface ExecutionReceipt {
  readonly binaryBlake3: string;
  readonly egressBytes: number;
  readonly dirtyBytes: number;
  readonly syscallCount?: number;
  readonly extensions?: ReceiptExtension[];
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
  readonly binary_path: string;
  readonly binary_blake3: string;
  readonly argv: string[];
  readonly cwd: string;
  readonly syscall_nr?: number;
  readonly prot_flags?: number;
  readonly memfd_flags?: number;
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
  readonly binary_path: string;
  readonly reason: string;
}
