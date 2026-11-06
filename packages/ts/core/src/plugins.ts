// see: cans/03-sdk-architecture.md#TypeScript-SDK

import type { KelirPlugin } from "./plugin.js";

export interface ChaosConfig {
  readonly network?: readonly {
    readonly target: string;
    readonly latency: { readonly baseMs: number; readonly jitterMs: number };
    readonly dropRate: number;
    readonly direction?: "inbound" | "outbound" | "both";
  }[];
  readonly fs?: readonly {
    readonly targetPath: string;
    readonly simulateError: string;
    readonly errorProbability: number;
  }[];
}

export function chaos(_config: ChaosConfig): KelirPlugin {
  return { name: "chaos" };
}

export interface LedgerConfig {
  readonly store?: string;
  readonly merkleChain?: boolean;
  readonly otel?: { readonly traceParent: boolean; readonly serviceName: string };
  readonly receipts?: {
    readonly includeDirtyBytes?: boolean;
    readonly includeBinaryBlake3?: boolean;
    readonly includeSyscallCount?: boolean;
  };
}

export function ledger(_config: LedgerConfig): KelirPlugin {
  return { name: "ledger" };
}

export interface McpConfig {
  readonly transport: {
    readonly streamKind: string;
    readonly framing: string;
  };
  readonly advertise: { readonly tools: readonly string[] };
}

export function mcp(_config: McpConfig): KelirPlugin {
  return { name: "mcp" };
}

export interface ToolGuardConfig {
  readonly pinSchemas: boolean;
  readonly maxInvocationsPerTrace: number;
  readonly approvals: Record<string, { readonly action: string; readonly durationMs: number }>;
}

export function toolGuard(_config: ToolGuardConfig): KelirPlugin {
  return { name: "tool-guard" };
}

export interface WireIngressConfig {
  readonly listen: { readonly port: number; readonly protocol: string };
  readonly parking: { readonly maxBufferedBytes: number; readonly idleTimeoutMs: number };
  readonly wakeOnPacket: { readonly transportChannel: number };
  readonly backlog: { readonly maxConnections: number; readonly overflow: string };
}

export function wireIngress(_config: WireIngressConfig): KelirPlugin {
  return { name: "wire-ingress" };
}
