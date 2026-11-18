// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/15-operations.md#Multi-language-parity-gate
// see: cans/10-attestation.md#Execution-receipts
// see: cans/11-protocol.md#Wire-framing

export * from "./telemetry.attest.generated.js";
export * from "./generated/protocol.wire.generated.js";

import type { ExecutionReceipt, ExitKind } from "./telemetry.attest.generated.js";

export interface SpawnOptions {
  readonly cwd?: string;
  readonly params?: Record<string, unknown>;
  readonly input?: string | Uint8Array;
  readonly env?: Record<string, string>;
}

export interface SpawnResult {
  readonly exitCode: number;
  readonly stdout?: Uint8Array;
  readonly stderr?: Uint8Array;
  readonly receipt?: ExecutionReceipt;
  readonly exitKind?: ExitKind;
}

export interface TrapInfo {
  readonly pid: number;
  readonly ppid: number;
  readonly binaryPath: string;
  readonly binaryBlake3: string;
  readonly argv: string[];
  readonly cwd: string;
  readonly treeDepth: number;
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

// see: cans/artifacts/code/plugin-trait.rs#FsInterposer
export interface DirEntry {
  readonly guestInode: number;
  readonly name: string;
}

export interface ExecveMutation {
  readonly binary?: string;
  readonly argv?: string[];
  readonly env?: Record<string, string>;
  readonly unset_env?: string[];
}

export interface CustomActionPayload {
  readonly name: string;
  readonly cbor: unknown;
}

// see: cans/05-storage-plane.md#Interception-pipeline
export interface FsPathRedirect {
  readonly targetPath: string;
}

export type PatchInput = ExecveMutation | CustomActionPayload | FsInodeAttr | FsPathRedirect;

// see: cans/02-plugin-model.md#Unified-verdict
export const PatchPayload: {
  FsPathRedirect(attr: { readonly targetPath: string }): FsPathRedirect;
} = {
  FsPathRedirect(attr: { readonly targetPath: string }): FsPathRedirect {
    return { targetPath: attr.targetPath };
  },
};

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
  readonly tree_depth?: number;
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
    treeDepth: raw.tree_depth ?? 0,
    syscallNr: raw.syscall_nr,
    protFlags: raw.prot_flags,
    memfdFlags: raw.memfd_flags,
  };
}
