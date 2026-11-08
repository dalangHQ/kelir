// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/15-operations.md#Multi-language-parity-gate
// see: cans/10-attestation.md#Execution-receipts
// see: cans/11-protocol.md#Wire-framing

export * from "./generated/telemetry.js";
export * from "./generated/protocol.js";

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
  readonly binaryPath: string;
  readonly binaryBlake3: string;
  readonly argv: string[];
  readonly cwd: string;
  readonly syscallNr?: number;
  readonly protFlags?: number;
  readonly memfdFlags?: number;
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
  readonly binaryPath: string;
  readonly reason: string;
}

// see: cans/15-operations.md#Multi-language-parity-gate
export interface NativeTrapInfoRecord {
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

export function fromNativeTrapInfo(raw: NativeTrapInfoRecord): TrapInfo {
  return {
    pid: raw.pid,
    ppid: raw.ppid,
    binaryPath: raw.binary_path,
    binaryBlake3: raw.binary_blake3,
    argv: raw.argv,
    cwd: raw.cwd,
    syscallNr: raw.syscall_nr,
    protFlags: raw.prot_flags,
    memfdFlags: raw.memfd_flags,
  };
}
